use ratatui::{
    layout::{Constraint, Direction, Layout, Rect, Alignment},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Cell, Paragraph, Row, Table, List, ListItem, Clear, Padding, Tabs, Wrap},
    Frame,
};
use super::app::{App, ViewMode, ActivePanel};

pub fn draw_ui(f: &mut Frame, app: &mut App<'_>) {
    app.refresh_sys_if_needed();
    let size = f.size();

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Tabs
            Constraint::Length(3),  // Header / System Stats
            Constraint::Min(10),    // Orta Alan (Tablo + Monitör)
            Constraint::Length(if app.active_tab == 0 { 8 } else { 0 }),  // Loglar (Sadece Dashboard'da)
            Constraint::Length(3),  // Komutlar (Footer)
        ])
        .split(size);

    // 0. TABS
    let titles = vec![" DASHBOARD ", " SYSTEM LOGS ", " SETTINGS "].into_iter().map(Line::from).collect();
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Plain).title(" NAVIGATION "))
        .select(app.active_tab)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
        .divider(" | ");
    f.render_widget(tabs, main_layout[0]);

    // 1. Header (Kaynak Kullanımı & HFT Milisaniye Saat)
    let pid = sysinfo::Pid::from_u32(std::process::id());
    let (cpu_usage, used_mem) = if let Some(p) = app.sys.process(pid) {
        (p.cpu_usage(), p.memory() / 1024 / 1024)
    } else {
        (0.0, 0)
    };

    let now = chrono::Local::now();
    let ms = now.timestamp_subsec_millis();
    let us = now.timestamp_subsec_micros() % 1000;
    let time_str = format!("{}.{:03}.{:03}", now.format("%H.%M.%S"), ms, us);

    let header_text = vec![
        Line::from(vec![
            Span::styled(" ENTERPRISE ORCHESTRATION CONSOLE ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(" | 🕒 SAAT: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{} ", time_str), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled("| CPU: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.1}% ", cpu_usage), Style::default().fg(Color::White)),
            Span::styled("| RAM: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{} MB ", used_mem), Style::default().fg(Color::White)),
        ])
    ];

    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Plain).border_style(Style::default().fg(Color::DarkGray)))
        .alignment(Alignment::Center);
    f.render_widget(header, main_layout[1]);

    if app.active_tab == 0 {
        // DASHBOARD VIEW
        let middle_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(app.systems_panel_width), 
                Constraint::Percentage(100 - app.systems_panel_width),
            ])
            .split(main_layout[2]);

        let monitor_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33), 
                Constraint::Percentage(33), 
                Constraint::Percentage(34),
            ])
            .split(middle_layout[1]);
            
        // Sistem Listesi
        let systems = app.orchestrator.list_systems();
        let rows: Vec<Row> = systems.iter().enumerate().map(|(i, (id, _name, running))| {
            let (bg, pointer) = if i == app.selected {
                (Color::Rgb(30, 30, 60), "▶ ")
            } else {
                (Color::Reset, "  ")
            };
            
            let status = if *running { "AKTİF" } else { "PASİF" };
            let status_color = if *running { Color::LightGreen } else { Color::LightRed };
            let id_color = if *running { Color::White } else { Color::DarkGray };
            
            let bytes_len = app.orchestrator.monitor_data(id).map(|d| d.len()).unwrap_or(0);
            let ram_kb = (bytes_len / 1024).max(16);
            let cpu_usage = if *running { (0.2 + (i as f32 * 0.15) * 10.0).round() / 10.0 } else { 0.0 };

            let actions = Line::from(vec![
                Span::styled(" [ START ] ", Style::default().fg(Color::White).bg(Color::DarkGray)),
                Span::styled(" ", Style::default()),
                Span::styled(" [ STOP ] ", Style::default().fg(Color::White).bg(Color::Rgb(60, 60, 60))),
                Span::styled(" ", Style::default()),
                Span::styled(" [ VIEW ] ", Style::default().fg(Color::White).bg(Color::DarkGray)),
                Span::styled(" ", Style::default()),
                Span::styled(" [ DEL ] ", Style::default().fg(Color::White).bg(Color::Rgb(60, 60, 60))),
            ]);
            
            Row::new(vec![
                Cell::from(format!("{}{}", pointer, id)).style(Style::default().fg(id_color)),
                Cell::from(status).style(Style::default().fg(status_color)),
                Cell::from(format!("{} KB", ram_kb)).style(Style::default().fg(Color::Cyan)),
                Cell::from(format!("{:.1}%", cpu_usage)).style(Style::default().fg(Color::Yellow)),
                Cell::from(actions),
            ])
            .style(Style::default().bg(bg))
        }).collect();

        let table = Table::new(rows)
            .header(Row::new(vec![" MODULE ID", " STATUS", " RAM", " CPU", " ACTIONS"])
                .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
                .bottom_margin(1))
            .block(Block::default()
                .title(Span::styled(" SYSTEM MODULES ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)))
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(Style::default().fg(if app.active_panel == ActivePanel::Systems { Color::Cyan } else if app.is_dragging_split { Color::White } else { Color::DarkGray }))
                .padding(Padding::horizontal(1)))
            .widths(&[Constraint::Percentage(35), Constraint::Percentage(15), Constraint::Percentage(50)])
            .column_spacing(1);
        f.render_widget(table, middle_layout[0]);

        // Data Inspector (Hex)
        let hex_content = if let Some(data) = &app.monitored_data {
            if data.is_empty() {
                vec![Line::from(Span::styled("Veri yok.", Style::default().fg(Color::DarkGray)))]
            } else {
                let mut lines = Vec::new();
                lines.push(Line::from(Span::styled(format!("Boyut: {} bytes", data.len()), Style::default().fg(Color::Gray))));
                lines.push(Line::from(""));
                for chunk in data.chunks(16) {
                    let hex: Vec<String> = chunk.iter().map(|b| format!("{:02x}", b)).collect();
                    let ascii: String = chunk.iter().map(|&b| if b >= 32 && b <= 126 { b as char } else { '.' }).collect();
                    lines.push(Line::from(vec![
                        Span::styled(format!("{:<48} ", hex.join(" ")), Style::default().fg(Color::Rgb(0, 255, 65))),
                        Span::styled(ascii, Style::default().fg(Color::Rgb(0, 255, 65))),
                    ]));
                }
                lines
            }
        } else {
            vec![Line::from(Span::styled("İzlemek için sistem seçip 'm' tuşuna basın.", Style::default().fg(Color::DarkGray)))]
        };
        
        let inspector = Paragraph::new(hex_content)
            .scroll((app.hex_scroll, 0))
            .block(Block::default()
                .title(Span::styled(" RAW DATA (HEX) ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)))
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(Style::default().fg(if app.active_panel == ActivePanel::Hex { Color::Cyan } else { Color::DarkGray }))
                .padding(Padding::horizontal(1)));
        f.render_widget(inspector, monitor_layout[0]);

        // Data Inspector (Text)
        let text_content = if let Some(data) = &app.monitored_data {
            if data.is_empty() {
                vec![Line::from(Span::styled("Veri yok.", Style::default().fg(Color::DarkGray)))]
            } else {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(data) {
                    // JSON ise güzelce formatla ve ekran gecikmesini hesapla
                    let mut lines = Vec::new();
                    let current_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
                    
                    if let Some(obj) = json.as_object() {
                        let symbol = obj.get("symbol").and_then(|v| v.as_str()).unwrap_or("UNKNOWN");
                        lines.push(Line::from(Span::styled(format!("🚀 {} Canlı Veri", symbol), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))));
                        lines.push(Line::from(""));
                        
                        if obj.contains_key("best_bid") {
                            let bid = obj.get("best_bid").and_then(|v| v.as_str()).unwrap_or("");
                            let bid_qty = obj.get("best_bid_qty").and_then(|v| v.as_str()).unwrap_or("");
                            let ask = obj.get("best_ask").and_then(|v| v.as_str()).unwrap_or("");
                            let ask_qty = obj.get("best_ask_qty").and_then(|v| v.as_str()).unwrap_or("");
                            let spread = obj.get("spread").and_then(|v| v.as_str()).unwrap_or("");
                            
                            lines.push(Line::from(vec![
                                Span::raw("Alış : "), Span::styled(format!("{} (Miktar: {})", bid, bid_qty), Style::default().fg(Color::Green)),
                            ]));
                            lines.push(Line::from(vec![
                                Span::raw("Satış: "), Span::styled(format!("{} (Miktar: {})", ask, ask_qty), Style::default().fg(Color::Red)),
                            ]));
                            lines.push(Line::from(vec![Span::raw("Fark : "), Span::styled(spread.to_string(), Style::default().fg(Color::Cyan))]));
                        } else if obj.contains_key("price") {
                            let price = obj.get("price").and_then(|v| v.as_str()).unwrap_or("");
                            let quantity = obj.get("quantity").and_then(|v| v.as_str()).unwrap_or("");
                            let is_buyer_maker = obj.get("is_buyer_maker").and_then(|v| v.as_bool()).unwrap_or(false);
                            
                            let color = if is_buyer_maker { Color::Red } else { Color::Green };
                            let side = if is_buyer_maker { "SATIM" } else { "ALIM " };
                            
                            lines.push(Line::from(vec![
                                Span::raw("İşlem : "), Span::styled(format!("{} @ {} (Miktar: {})", side, price, quantity), Style::default().fg(color)),
                            ]));
                        } else if obj.contains_key("mark_price") {
                            let mark_price = obj.get("mark_price").and_then(|v| v.as_str()).unwrap_or("");
                            let index_price = obj.get("index_price").and_then(|v| v.as_str()).unwrap_or("");
                            let funding_rate = obj.get("funding_rate").and_then(|v| v.as_str()).unwrap_or("");
                            
                            lines.push(Line::from(vec![
                                Span::raw("Mark Fiyatı : "), Span::styled(mark_price.to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                            ]));
                            lines.push(Line::from(vec![
                                Span::raw("Endeks Fiyat: "), Span::styled(index_price.to_string(), Style::default().fg(Color::Cyan)),
                            ]));
                            lines.push(Line::from(vec![
                                Span::raw("Fonlama Oranı: "), Span::styled(funding_rate.to_string(), Style::default().fg(Color::LightMagenta)),
                            ]));
                        } else if obj.contains_key("type") && obj.get("type").and_then(|v| v.as_str()) == Some("ohlcv") {
                            let symbol = obj.get("symbol").and_then(|v| v.as_str()).unwrap_or("");
                            let interval = obj.get("interval").and_then(|v| v.as_str()).unwrap_or("");
                            
                            lines.push(Line::from(vec![
                                Span::raw("Veri Tipi: "), Span::styled("OHLCV Mumları", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                            ]));
                            lines.push(Line::from(vec![
                                Span::raw("Parametre: "), Span::styled(format!("{} - {}", symbol, interval), Style::default().fg(Color::Cyan)),
                            ]));
                            
                            if let Some(arr) = obj.get("data").and_then(|v| v.as_array()) {
                                lines.push(Line::from(format!("{} adet mum çekildi.", arr.len())));
                                for (i, kline) in arr.iter().enumerate().take(5) {
                                    if let Some(k) = kline.as_array() {
                                        let open = k[1].as_str().unwrap_or("");
                                        let high = k[2].as_str().unwrap_or("");
                                        let low = k[3].as_str().unwrap_or("");
                                        let close = k[4].as_str().unwrap_or("");
                                        let volume = k[5].as_str().unwrap_or("");
                                        lines.push(Line::from(format!("[{}] O:{} | H:{} | L:{} | C:{} | V:{}", i, open, high, low, close, volume)));
                                    }
                                }
                                if arr.len() > 5 {
                                    lines.push(Line::from("..."));
                                }
                            }
                        }
                        lines.push(Line::from(""));
                        lines.push(Line::from(Span::styled("Ham JSON:", Style::default().fg(Color::DarkGray))));
                        let pretty_json = serde_json::to_string_pretty(&json).unwrap_or_default();
                        for l in pretty_json.lines() {
                            lines.push(Line::from(Span::styled(l.to_string(), Style::default().fg(Color::DarkGray))));
                        }
                    }
                    lines
                } else {
                    let s = String::from_utf8_lossy(data);
                    s.lines().map(|l| Line::from(Span::styled(l.to_string(), Style::default().fg(Color::LightGreen)))).collect()
                }
            }
        } else {
            vec![Line::from(Span::styled("Bekleniyor...", Style::default().fg(Color::DarkGray)))]
        };
        
        let text_inspector = Paragraph::new(text_content)
            .scroll((app.live_feed_scroll, 0))
            .block(Block::default()
                .title(Span::styled(" LIVE DATA FEED ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)))
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(Style::default().fg(if app.active_panel == ActivePanel::LiveFeed { Color::Cyan } else { Color::DarkGray }))
                .padding(Padding::horizontal(1)))
            .wrap(ratatui::widgets::Wrap { trim: true });
        f.render_widget(text_inspector, monitor_layout[1]);

        // Shell (Komut İstemi) Sağ tarafta sabit
        let shell_border_color = if app.active_panel == ActivePanel::Shell || app.mode == ViewMode::Shell { Color::Cyan } else { Color::DarkGray };
        let shell_title_color = if app.active_panel == ActivePanel::Shell || app.mode == ViewMode::Shell { Color::Cyan } else { Color::White };
        let mut shell_lines = Vec::new();
        shell_lines.push(Line::from(""));
        
        let history_start = if app.shell_history.len() > 10 { app.shell_history.len() - 10 } else { 0 };
        for cmd in app.shell_history.iter().skip(history_start) {
            shell_lines.push(Line::from(vec![
                Span::styled(" > ", Style::default().fg(Color::DarkGray)),
                Span::raw(cmd)
            ]));
        }
        
        shell_lines.push(Line::from(""));
        
        // Current input
        shell_lines.push(Line::from(vec![
            Span::styled(" > ", Style::default().fg(if app.mode == ViewMode::Shell { Color::Green } else { Color::DarkGray }).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{}_", app.input_shell), Style::default().fg(Color::White)),
        ]));
        
        shell_lines.push(Line::from(""));
        shell_lines.push(Line::from(Span::styled(" 'i' ile Shell'e geç. Shell'deyken ESC ile çık.", Style::default().fg(Color::DarkGray))));

        let p = Paragraph::new(shell_lines)
            .block(Block::default()
                .title(Span::styled(" COMMAND SHELL ", Style::default().fg(shell_title_color).add_modifier(Modifier::BOLD)))
                .borders(Borders::ALL)
                .border_type(if app.mode == ViewMode::Shell { BorderType::Rounded } else { BorderType::Plain })
                .border_style(Style::default().fg(shell_border_color))
                .padding(Padding::horizontal(1)))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });
            
        f.render_widget(p, monitor_layout[2]);

        // Loglar
        let max_lines = main_layout[3].height.saturating_sub(2) as usize; 
        let base_skip = if app.logs.len() > max_lines { app.logs.len() - max_lines } else { 0 };
        let skip = base_skip.saturating_sub(app.logs_scroll as usize);
        let take = max_lines.min(app.logs.len() - skip);
        let logs_to_show = &app.logs[skip..skip + take];
        let log_items: Vec<ListItem> = logs_to_show.iter().map(|msg| {
            ListItem::new(Line::from(Span::styled(msg, Style::default().fg(Color::Gray))))
        }).collect();

        let log_list = List::new(log_items)
            .block(Block::default()
                .title(Span::styled(" SYSTEM EVENTS ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)))
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(Style::default().fg(if app.active_panel == ActivePanel::Logs { Color::Cyan } else { Color::DarkGray }))
                .padding(Padding::horizontal(1)));
        f.render_widget(log_list, main_layout[3]);
        
    } else if app.active_tab == 1 {
        // TAM EKRAN LOGLAR
        let log_items: Vec<ListItem> = app.logs.iter().map(|msg| {
            ListItem::new(Line::from(Span::styled(msg, Style::default().fg(Color::LightCyan))))
        }).collect();
        let log_list = List::new(log_items)
            .block(Block::default()
                .title(Span::styled(" 📜 Detaylı Loglar ", Style::default().fg(Color::LightMagenta).add_modifier(Modifier::BOLD)))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
                .padding(Padding::horizontal(2)));
        f.render_widget(log_list, main_layout[2]);
    } else {
        // AYARLAR
        let web_status_span = if app.web_server_started {
            Span::styled(" 🚀 ÇALIŞIYOR (http://localhost:8080) ", Style::default().fg(Color::Black).bg(Color::LightGreen).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" ⏹️ KAPALI (OFF) ", Style::default().fg(Color::White).bg(Color::Red).add_modifier(Modifier::BOLD))
        };

        let text = vec![
            Line::from(Span::styled("⚙ Ayarlar & Sistem Web Sunucusu Kontrolü", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from(vec![
                Span::styled("Port 8080 Web Sunucu Durumu: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                web_status_span,
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" [W] ", Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(
                    if app.web_server_started { " ⏹️ WEB ARAYÜZÜNÜ DURDUR (Port 8080 Kapat)" } else { " 🚀 WEB ARAYÜZÜNÜ BAŞLAT (Port 8080 Aç)" },
                    Style::default().fg(if app.web_server_started { Color::LightRed } else { Color::Cyan }).add_modifier(Modifier::BOLD)
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" [E] ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(" Web Arayüzünü Tarayıcıda Aç (http://localhost:8080)", Style::default().fg(Color::LightCyan)),
            ]),
            Line::from(vec![
                Span::styled(" [T] ", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::styled(" Terminal İçi Metin Editörü (tui-textarea)", Style::default().fg(Color::Green)),
            ]),
            Line::from(""),
            Line::from(Span::styled("Sistem başlatıldığında varsayılan olarak SADECE bu TUI ekranı çalışır.", Style::default().fg(Color::White))),
            Line::from(Span::styled("[W] tuşuna basarak Port 8080 Web Sunucusunu dilediğiniz an BAŞLATABİLİR veya KAPATABİLİRSİNİZ.", Style::default().fg(Color::LightYellow))),
            Line::from(""),
            Line::from(Span::styled("Sistem çalışırken ayarları değiştirdiğinizde Hot-Reload ile motor anında güncellenir.", Style::default().fg(Color::DarkGray))),
        ];
        let p = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" ⚙ Ayarlar "))
            .alignment(Alignment::Center);
        f.render_widget(p, main_layout[2]);
    }

    // Config Editor Popup
    if app.mode == ViewMode::ConfigEditor {
        if let Some(ref mut ta) = app.textarea {
            let popup_area = centered_rect(80, 80, size);
            f.render_widget(Clear, popup_area);
            
            ta.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Thick)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(" 📝 Config Editor (flow_config.json) ")
            );
            ta.set_style(Style::default().fg(Color::White).bg(Color::Reset));
            ta.set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));
            
            f.render_widget(ta.widget(), popup_area);
            
            // Editor Help
            let help_area = Rect {
                x: popup_area.x,
                y: popup_area.y + popup_area.height,
                width: popup_area.width,
                height: 1,
            };
            let help_text = Paragraph::new(Line::from(vec![
                Span::styled(" [Ctrl+S] Kaydet ve Uygula ", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw("   "),
                Span::styled(" [ESC] İptal ", Style::default().fg(Color::White).bg(Color::Red).add_modifier(Modifier::BOLD)),
            ])).alignment(Alignment::Center);
            
            f.render_widget(help_text, help_area);
        }
    }

    let help_line = Line::from(vec![
        Span::styled("   ", Style::default()), // offset x=3
        Span::styled(" [+ Yeni Eklenti Yükle] ", Style::default().fg(Color::White).bg(Color::Rgb(150, 150, 40)).add_modifier(Modifier::BOLD)),
        Span::styled("  ", Style::default()),
        Span::styled(" [Q] Çıkış ", Style::default().fg(Color::White).bg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Span::styled("    (Tam GUI Kontrolü)", Style::default().fg(Color::DarkGray)),
    ]);
    let help = Paragraph::new(help_line)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(Color::DarkGray)))
        .alignment(Alignment::Left);
    f.render_widget(help, main_layout[4]);

    // Popup (Eklenti Seçimi)
    if app.mode == ViewMode::PluginSelection {
        let popup_area = centered_rect(40, 60, size);
        f.render_widget(Clear, popup_area);
        let items: Vec<ListItem> = app.available_plugins.iter().enumerate().map(|(i, p)| {
            let (bg, prefix) = if i == app.plugin_selected { (Color::Rgb(50, 50, 100), "▶ ") } else { (Color::Reset, "  ") };
            ListItem::new(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::Yellow)),
                Span::raw(p.clone())
            ])).style(Style::default().bg(bg).fg(Color::White))
        }).collect();
        let list = List::new(items)
            .block(Block::default().title(Span::styled(" 📦 Eklenti Yükle (Sol Tık) ", Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD)))
                .borders(Borders::ALL).border_type(BorderType::Thick).border_style(Style::default().fg(Color::LightYellow)).padding(Padding::new(2, 2, 1, 1)));
        f.render_widget(list, popup_area);
    }
    
    // Onay Penceresi (Confirm Delete Modal)
    if let ViewMode::ConfirmDelete(ref sys_id) = app.mode {
        let modal_area = centered_rect(30, 20, size);
        f.render_widget(Clear, modal_area);
        let text = vec![
            Line::from(Span::styled(format!("'{}'", sys_id), Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD))),
            Line::from("Sistemini silmek istediğinize"),
            Line::from("emin misiniz?"),
            Line::from(""),
            Line::from(vec![
                Span::styled("  [ EVET ]  ", Style::default().fg(Color::White).bg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw("    "),
                Span::styled("  [ HAYIR ]  ", Style::default().fg(Color::White).bg(Color::DarkGray).add_modifier(Modifier::BOLD)),
            ]),
        ];
        let p = Paragraph::new(text)
            .block(Block::default().title(" ⚠️ Dikkat ").borders(Borders::ALL).border_type(BorderType::Thick).border_style(Style::default().fg(Color::Red)).padding(Padding::vertical(1)))
            .alignment(Alignment::Center);
        f.render_widget(p, modal_area);
    }
    


    
    // Sağ Tık İçerik Menüsü (Context Menu)
    if let ViewMode::ContextMenu(ref id, cx, cy) = app.mode {
        // Small popup at cx, cy
        let area = Rect {
            x: cx,
            y: cy,
            width: 25,
            height: 6,
        };
        // Ensure it doesn't overflow screen
        let area = Rect {
            x: area.x.min(size.width.saturating_sub(25)),
            y: area.y.min(size.height.saturating_sub(6)),
            width: 25,
            height: 6,
        };
        f.render_widget(Clear, area);
        let text = vec![
            Line::from(Span::styled(format!(" ⚙ {}", id), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled("  ▶ Başlat ", Style::default().fg(Color::White))),
            Line::from(Span::styled("  ■ Durdur ", Style::default().fg(Color::White))),
            Line::from(Span::styled("  ✖ Sil ", Style::default().fg(Color::White))),
        ];
        let p = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(Color::DarkGray)).style(Style::default().bg(Color::Rgb(40,40,40))));
        f.render_widget(p, area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage((100 - percent_y) / 2), Constraint::Percentage(percent_y), Constraint::Percentage((100 - percent_y) / 2)])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage((100 - percent_x) / 2), Constraint::Percentage(percent_x), Constraint::Percentage((100 - percent_x) / 2)])
        .split(popup_layout[1])[1]
}
