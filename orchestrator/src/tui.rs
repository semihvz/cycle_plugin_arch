use ratatui::{
    layout::{Constraint, Direction, Layout, Rect, Alignment},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Cell, Paragraph, Row, Table, List, ListItem, Clear, Padding, Tabs},
    Frame,
};
use crate::{App, ViewMode};

pub fn draw_ui(f: &mut Frame, app: &mut App) {
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
    let titles = vec![" 🖥 Dashboard ", " 📜 Detaylı Loglar ", " ⚙ Ayarlar "].into_iter().map(Line::from).collect();
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" Navigasyon "))
        .select(app.active_tab)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD))
        .divider(" | ");
    f.render_widget(tabs, main_layout[0]);

    // 1. Header (Kaynak Kullanımı)
    let pid = sysinfo::Pid::from_u32(std::process::id());
    let (cpu_usage, used_mem) = if let Some(p) = app.sys.process(pid) {
        (p.cpu_usage(), p.memory() / 1024 / 1024)
    } else {
        (0.0, 0)
    };

    let header_text = vec![
        Line::from(vec![
            Span::styled(" 🚀 CYCLE-ORC | Orkestratör Paneli | ", Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD)),
            Span::styled("CPU: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.1}% ", cpu_usage), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("| RAM: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{} MB ", used_mem), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        ])
    ];

    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(Color::DarkGray)))
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
                Constraint::Percentage(50), 
                Constraint::Percentage(50), 
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
            
            let actions = Line::from(vec![
                Span::styled(" [▶ Başlat] ", Style::default().fg(Color::White).bg(Color::Rgb(40, 120, 60))),
                Span::styled(" ", Style::default()),
                Span::styled(" [■ Durdur] ", Style::default().fg(Color::White).bg(Color::Rgb(150, 40, 40))),
                Span::styled(" ", Style::default()),
                Span::styled(" [👁 İzle] ", Style::default().fg(Color::White).bg(Color::Rgb(40, 60, 150))),
                Span::styled(" ", Style::default()),
                Span::styled(" [✖ Sil] ", Style::default().fg(Color::White).bg(Color::Rgb(120, 40, 120))),
            ]);
            
            Row::new(vec![
                Cell::from(format!("{}{}", pointer, id)).style(Style::default().fg(id_color)),
                Cell::from(status).style(Style::default().fg(status_color)),
                Cell::from(actions),
            ])
            .style(Style::default().bg(bg))
        }).collect();

        let table = Table::new(rows)
            .header(Row::new(vec![" EKLENTİ ID", " DURUM", " İŞLEMLER"])
                .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
                .bottom_margin(1))
            .block(Block::default()
                .title(Span::styled(" 🧩 Sistemler ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if app.is_dragging_split { Color::Yellow } else { Color::DarkGray }))
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
                        Span::styled(format!("{:<48} ", hex.join(" ")), Style::default().fg(Color::Rgb(100, 150, 255))),
                        Span::styled(ascii, Style::default().fg(Color::Rgb(200, 200, 100))),
                    ]));
                }
                lines
            }
        } else {
            vec![Line::from(Span::styled("İzlemek için sistem seçip 'm' tuşuna basın.", Style::default().fg(Color::DarkGray)))]
        };
        
        let inspector = Paragraph::new(hex_content)
            .scroll((app.monitor_scroll, 0))
            .block(Block::default()
                .title(Span::styled(" 🔍 Canlı Veri (Hex) ", Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD)))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
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
                        lines.push(Line::from(""));
                        
                        // Metrikler
                        lines.push(Line::from(Span::styled("⚡ HFT Gecikme Metrikleri", Style::default().fg(Color::LightMagenta).add_modifier(Modifier::BOLD))));
                        
                        let exchange_lat = obj.get("exchange_latency_ms").and_then(|v| v.as_i64()).unwrap_or(0);
                        let proc_lat = obj.get("processing_latency_us").and_then(|v| v.as_i64()).unwrap_or(0);
                        let db_lat = obj.get("db_write_latency_us").and_then(|v| v.as_i64()).unwrap_or(0);
                        let write_time = obj.get("local_write_time_ms").and_then(|v| v.as_i64()).unwrap_or(current_time);
                        let screen_delay = current_time.saturating_sub(write_time);
                        let e2e_delay = exchange_lat + (proc_lat / 1000) + screen_delay;

                        lines.push(Line::from(format!("  - Borsa (Exchange) Gecikmesi : {} ms", exchange_lat)));
                        lines.push(Line::from(format!("  - Plugin İşleme Gecikmesi    : {} µs (mikrosaniye)", proc_lat)));
                        lines.push(Line::from(format!("  - SQLite DB Yazma Gecikmesi  : {} µs", db_lat)));
                        lines.push(Line::from(format!("  - RAM -> Ekran Gecikmesi     : {} ms", screen_delay)));
                        lines.push(Line::from(Span::styled(format!("  - Toplam Uçtan Uca Gecikme   : {} ms", e2e_delay), Style::default().fg(Color::Yellow))));
                        
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
            .scroll((app.monitor_scroll, 0))
            .block(Block::default()
                .title(Span::styled(" 📄 Canlı Veri (Okunabilir) ", Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
                .padding(Padding::horizontal(1)))
            .wrap(ratatui::widgets::Wrap { trim: true });
        f.render_widget(text_inspector, monitor_layout[1]);

        // Loglar
        let max_lines = main_layout[3].height.saturating_sub(2) as usize; 
        let skip = if app.logs.len() > max_lines { app.logs.len() - max_lines } else { 0 };
        let logs_to_show = &app.logs[skip..];
        let log_items: Vec<ListItem> = logs_to_show.iter().map(|msg| {
            ListItem::new(Line::from(Span::styled(msg, Style::default().fg(Color::Gray))))
        }).collect();

        let log_list = List::new(log_items)
            .block(Block::default()
                .title(Span::styled(" 📜 Sistem Olay Günlüğü ", Style::default().fg(Color::LightMagenta).add_modifier(Modifier::BOLD)))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
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
        let text = Paragraph::new(Line::from(Span::styled("Ayarlar menüsü (Gelecek Özellik)", Style::default().fg(Color::DarkGray))))
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" ⚙ Ayarlar "))
            .alignment(Alignment::Center);
        f.render_widget(text, main_layout[2]);
    }

    // Footer
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
