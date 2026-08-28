#!/usr/bin/env python3
import os
import re
import markdown

def convert_md_to_html(md_path, html_path):
    with open(md_path, 'r', encoding='utf-8') as f:
        md_content = f.read()

    # Convert markdown code blocks with ```mermaid to <pre class="mermaid">...
    def replace_mermaid(match):
        code = match.group(1)
        return f'<div class="mermaid-container"><pre class="mermaid">\n{code}\n</pre></div>'

    processed_md = re.sub(r'```mermaid\s*\n(.*?)```', replace_mermaid, md_content, flags=re.DOTALL)

    # Convert markdown to HTML
    html_body = markdown.markdown(
        processed_md,
        extensions=[
            'tables',
            'fenced_code',
            'codehilite',
            'toc',
            'attr_list',
            'def_list'
        ]
    )

    # Explicitly map H2 section titles to short, bulletproof clean IDs
    id_mapping = [
        (r'1\.\s*Genel Bakış.*', 'genel-bakis'),
        (r'2\.\s*Sistem Katmanları.*', 'katmanlar'),
        (r'3\.\s*Veri Akışı.*', 'veri-akisi'),
        (r'4\.\s*Donanım.*', 'optimizasyon'),
        (r'5\.\s*Yapılandırma.*', 'topoloji'),
        (r'6\.\s*Derleme.*', 'derleme'),
        (r'7\.\s*Market Structure.*', 'kirilim-modeli'),
        (r'8\.\s*Özet.*', 'ozet')
    ]

    def fix_h2_id(match):
        title_text = match.group(1)
        clean_text = re.sub(r'<[^>]+>', '', title_text).strip()
        assigned_id = 'section'
        for pattern, custom_id in id_mapping:
            if re.search(pattern, clean_text, re.IGNORECASE):
                assigned_id = custom_id
                break
        return f'<h2 id="{assigned_id}">{title_text}</h2>'

    html_body = re.sub(r'<h2[^>]*>(.*?)</h2>', fix_h2_id, html_body)

    full_html = f"""<!DOCTYPE html>
<html lang="tr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Cycle Orchestrator (cycle-orc) - Sistem Mimarisi & Piyasa Dinamikleri Dokümantasyonu</title>
    
    <!-- Google Fonts -->
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=Fira+Code:wght@400;500;600&family=Inter:wght@300;400;500;600;700;800&display=swap" rel="stylesheet">
    
    <!-- Mermaid.js for interactive diagrams -->
    <script src="https://cdn.jsdelivr.net/npm/mermaid/dist/mermaid.min.js"></script>
    <script>
        document.addEventListener("DOMContentLoaded", function() {{
            mermaid.initialize({{
                startOnLoad: true,
                theme: 'dark',
                securityLevel: 'loose',
                themeVariables: {{
                    fontFamily: 'Inter, sans-serif',
                    primaryColor: '#00d2ff',
                    primaryTextColor: '#ffffff',
                    primaryBorderColor: '#00d2ff',
                    lineColor: '#00e5ff',
                    secondaryColor: '#1e293b',
                    tertiaryColor: '#0f172a'
                }}
            }});
        }});
    </script>

    <!-- MathJax for rendering LaTeX math formulas -->
    <script src="https://polyfill.io/v3/polyfill.min.js?features=es6"></script>
    <script id="MathJax-script" async src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js"></script>

    <style>
        :root {{
            --bg-primary: #0b0f19;
            --bg-secondary: #111827;
            --bg-card: #1f2937;
            --border-color: #374151;
            --accent-color: #00d2ff;
            --accent-gradient: linear-gradient(135deg, #00d2ff 0%, #3a7bd5 100%);
            --text-primary: #f9fafb;
            --text-secondary: #9ca3af;
            --text-muted: #6b7280;
            --code-bg: #111827;
            --success-color: #10b981;
            --warning-color: #f59e0b;
            --danger-color: #ef4444;
        }}

        html {{
            scroll-behavior: smooth;
        }}

        * {{
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }}

        body {{
            font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background-color: var(--bg-primary);
            color: var(--text-primary);
            line-height: 1.7;
            padding: 0;
            margin: 0;
            font-size: 16px;
        }}

        .top-navbar {{
            position: sticky;
            top: 0;
            z-index: 1000;
            background: rgba(11, 15, 25, 0.92);
            backdrop-filter: blur(16px);
            border-bottom: 1px solid var(--border-color);
            padding: 1rem 2rem;
            display: flex;
            align-items: center;
            justify-content: space-between;
        }}

        .brand {{
            display: flex;
            align-items: center;
            gap: 12px;
            font-weight: 700;
            font-size: 1.15rem;
            color: #fff;
            text-decoration: none;
        }}

        .brand-badge {{
            background: var(--accent-gradient);
            color: #000;
            font-size: 0.75rem;
            font-weight: 800;
            padding: 4px 10px;
            border-radius: 20px;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        }}

        .nav-links {{
            display: flex;
            gap: 0.8rem;
            list-style: none;
        }}

        .nav-links a {{
            color: var(--text-secondary);
            text-decoration: none;
            font-weight: 600;
            font-size: 0.9rem;
            padding: 6px 12px;
            border-radius: 8px;
            transition: all 0.2s ease;
        }}

        .nav-links a:hover {{
            color: #ffffff;
            background-color: rgba(0, 210, 255, 0.2);
        }}

        .container {{
            max-width: 1200px;
            margin: 0 auto;
            padding: 3rem 2rem 5rem 2rem;
        }}

        h1, h2, h3, h4 {{
            scroll-margin-top: 100px;
        }}

        h1 {{
            font-size: 2.5rem;
            font-weight: 800;
            background: var(--accent-gradient);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            margin-bottom: 1rem;
            line-height: 1.2;
        }}

        h2 {{
            font-size: 1.75rem;
            font-weight: 700;
            color: #ffffff;
            margin-top: 3.5rem;
            margin-bottom: 1.25rem;
            padding-bottom: 0.5rem;
            border-bottom: 1px solid rgba(255, 255, 255, 0.1);
        }}

        h3 {{
            font-size: 1.35rem;
            font-weight: 600;
            color: var(--accent-color);
            margin-top: 2rem;
            margin-bottom: 1rem;
        }}

        h4 {{
            font-size: 1.1rem;
            font-weight: 600;
            color: var(--text-primary);
            margin-top: 1.5rem;
            margin-bottom: 0.75rem;
        }}

        p {{
            margin-bottom: 1.25rem;
            color: #d1d5db;
        }}

        blockquote {{
            background: rgba(0, 210, 255, 0.05);
            border-left: 4px solid var(--accent-color);
            padding: 1.25rem 1.5rem;
            border-radius: 0 12px 12px 0;
            margin: 1.5rem 0;
            color: #e5e7eb;
        }}

        ul, ol {{
            margin-bottom: 1.5rem;
            padding-left: 1.75rem;
            color: #d1d5db;
        }}

        li {{
            margin-bottom: 0.5rem;
        }}

        code {{
            font-family: 'Fira Code', monospace;
            background-color: var(--code-bg);
            color: #38bdf8;
            padding: 3px 8px;
            border-radius: 6px;
            font-size: 0.9em;
            border: 1px solid rgba(255, 255, 255, 0.08);
        }}

        pre {{
            background-color: var(--code-bg);
            border: 1px solid var(--border-color);
            border-radius: 12px;
            padding: 1.25rem;
            overflow-x: auto;
            margin: 1.5rem 0;
            box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.5);
        }}

        pre code {{
            background-color: transparent;
            padding: 0;
            border: none;
            color: #f3f4f6;
            font-size: 0.92em;
            line-height: 1.6;
        }}

        table {{
            width: 100%;
            border-collapse: separate;
            border-spacing: 0;
            margin: 2rem 0;
            border-radius: 12px;
            overflow: hidden;
            border: 1px solid var(--border-color);
            box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.3);
        }}

        th {{
            background-color: var(--bg-card);
            color: var(--accent-color);
            font-weight: 700;
            text-align: left;
            padding: 1rem 1.25rem;
            border-bottom: 1px solid var(--border-color);
            font-size: 0.95rem;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        }}

        td {{
            padding: 1rem 1.25rem;
            border-bottom: 1px solid rgba(255, 255, 255, 0.05);
            background-color: var(--bg-secondary);
            color: #e5e7eb;
        }}

        tr:last-child td {{
            border-bottom: none;
        }}

        tr:hover td {{
            background-color: rgba(255, 255, 255, 0.03);
        }}

        .mermaid-container {{
            background: var(--bg-secondary);
            border: 1px solid var(--border-color);
            border-radius: 16px;
            padding: 2rem;
            margin: 2rem 0;
            display: flex;
            justify-content: center;
            align-items: center;
            box-shadow: 0 10px 30px rgba(0, 0, 0, 0.4);
            overflow-x: auto;
        }}

        .mermaid {{
            width: 100%;
            text-align: center;
        }}

        a {{
            color: var(--accent-color);
            text-decoration: none;
            transition: all 0.2s ease;
        }}

        a:hover {{
            text-decoration: underline;
        }}

        hr {{
            border: none;
            border-top: 1px solid var(--border-color);
            margin: 3rem 0;
        }}

        .footer {{
            text-align: center;
            padding: 3rem 2rem;
            color: var(--text-muted);
            font-size: 0.9rem;
            border-top: 1px solid var(--border-color);
            background-color: var(--bg-secondary);
        }}
    </style>
</head>
<body>
    <header class="top-navbar">
        <a href="#" class="brand">
            ⚡ Cycle Orchestrator
            <span class="brand-badge">ARCHITECTURE & DYNAMICS</span>
        </a>
        <ul class="nav-links">
            <li><a href="#genel-bakis">Genel Bakış</a></li>
            <li><a href="#katmanlar">Katmanlar</a></li>
            <li><a href="#veri-akisi">Veri Akışı</a></li>
            <li><a href="#optimizasyon">Optimizasyon</a></li>
            <li><a href="#topoloji">Topoloji</a></li>
            <li><a href="#derleme">Derleme</a></li>
            <li><a href="#kirilim-modeli">Kırılım Modeli</a></li>
        </ul>
    </header>

    <main class="container">
        {html_body}
    </main>

    <footer class="footer">
        <p>&copy; 2026 Cycle Orchestrator (cycle-orc) - Tüm Hakları Saklıdır.</p>
        <p>Yüksek Frekanslı Kantitatif İşlem & Zero-Copy Plugin Orkestrasyon Altyapısı</p>
    </footer>
</body>
</html>
"""

    with open(html_path, 'w', encoding='utf-8') as f:
        f.write(full_html)

    print(f"Successfully converted {md_path} to {html_path}")

if __name__ == "__main__":
    md_file = "/home/smhvz/Desktop/cycle-orc/docs/ARCHITECTURE.md"
    html_file = "/home/smhvz/Desktop/cycle-orc/docs/ARCHITECTURE.html"
    convert_md_to_html(md_file, html_file)
