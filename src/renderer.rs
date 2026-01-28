use notionrs_types::prelude::*;

pub struct HtmlRenderer;

impl HtmlRenderer {
    pub fn render_block(block: &Block) -> String {
        match block {
            Block::Paragraph { paragraph } => {
                let text = Self::render_rich_text(&paragraph.rich_text);
                let color_part = Self::get_color_part(&paragraph.color);
                let color_class = if color_part.is_empty() { String::new() } else { format!("ColorfulBlock--{}", color_part) };
                format!("<p class=\"Paragraph {}\">{}</p>", color_class, text)
            }
            Block::Heading1 { heading_1 } => {
                let text = Self::render_rich_text(&heading_1.rich_text);
                let color_part = Self::get_color_part(&heading_1.color);
                let color_class = if color_part.is_empty() { String::new() } else { format!("ColorfulBlock--{}", color_part) };
                format!(
                    "<h1 class=\"Heading Heading--1 {}\">{}</h1>",
                    color_class, text
                )
            }
            Block::Heading2 { heading_2 } => {
                let text = Self::render_rich_text(&heading_2.rich_text);
                let color_part = Self::get_color_part(&heading_2.color);
                let color_class = if color_part.is_empty() { String::new() } else { format!("ColorfulBlock--{}", color_part) };
                format!(
                    "<h2 class=\"Heading Heading--2 {}\">{}</h2>",
                    color_class, text
                )
            }
            Block::Heading3 { heading_3 } => {
                let text = Self::render_rich_text(&heading_3.rich_text);
                let color_part = Self::get_color_part(&heading_3.color);
                let color_class = if color_part.is_empty() { String::new() } else { format!("ColorfulBlock--{}", color_part) };
                format!(
                    "<h3 class=\"Heading Heading--3 {}\">{}</h3>",
                    color_class, text
                )
            }
            Block::BulletedListItem { bulleted_list_item } => {
                let text = Self::render_rich_text(&bulleted_list_item.rich_text);
                let color_part = Self::get_color_part(&bulleted_list_item.color);
                let color_class = if color_part.is_empty() { String::new() } else { format!("ColorfulBlock--{}", color_part) };
                format!("<li class=\"BulletedList {}\">{}</li>", color_class, text)
            }
            Block::NumberedListItem { numbered_list_item } => {
                let text = Self::render_rich_text(&numbered_list_item.rich_text);
                let color_part = Self::get_color_part(&numbered_list_item.color);
                let color_class = if color_part.is_empty() { String::new() } else { format!("ColorfulBlock--{}", color_part) };
                format!("<li class=\"NumberedList {}\">{}</li>", color_class, text)
            }
            Block::Code { code } => {
                let text = Self::render_rich_text(&code.rich_text);
                format!("<pre class=\"Code\"><code>{}</code></pre>", text)
            }
            Block::Quote { quote } => {
                let text = Self::render_rich_text(&quote.rich_text);
                let color_part = Self::get_color_part(&quote.color);
                let color_class = if color_part.is_empty() { String::new() } else { format!("ColorfulBlock--{}", color_part) };
                format!(
                    "<blockquote class=\"Quote {}\">{}</blockquote>",
                    color_class, text
                )
            }
            Block::Callout { callout } => {
                let text = Self::render_rich_text(&callout.rich_text);
                let emoji = match &callout.icon {
                    Some(icon) => icon.to_string(),
                    None => "💡".to_string(),
                };
                let color_part = Self::get_color_part(&callout.color);
                let color_class = if color_part.is_empty() { String::new() } else { format!("ColorfulBlock--{}", color_part) };
                format!(
                    "<div class=\"Callout {}\"><div class=\"Callout__Icon\">{}</div><div class=\"Callout__Content\">{}</div></div>",
                    color_class, emoji, text
                )
            }
            Block::Image { image } => {
                let url = image.to_string();
                format!(
                    "<div class=\"Image Image--Normal\"><figure><img src=\"{}\" /><figcaption></figcaption></figure></div>",
                    url
                )
            }
            Block::Video { video } => {
                let url = video.to_string();
                format!(
                    "<div class=\"Video\"><div class=\"Video__Content\"><video controls src=\"{}\"></video></div></div>",
                    url
                )
            }
            Block::Audio { audio } => {
                let url = audio.to_string();
                format!(
                    "<div class=\"Audio\"><audio controls src=\"{}\"></audio></div>",
                    url
                )
            }
            Block::File { file } => {
                let url = file.to_string();
                // 仅提取文件名，避免暴露包含加密签名的长 URL
                let name = url.split('?').next().unwrap_or(&url); // 去掉查询参数
                let name = name.split('/').last().unwrap_or("Download File");
                format!(
                    "<div class=\"File\"><a href=\"{}\" target=\"_blank\">📎 {}</a></div>",
                    url, name
                )
            }
            Block::Pdf { pdf } => {
                let url = pdf.to_string();
                format!(
                    "<div class=\"Pdf\"><embed src=\"{}\" type=\"application/pdf\" width=\"100%\" height=\"500px\" /></div>",
                    url
                )
            }
            Block::Embed { embed } => {
                let url = embed.url.clone();
                format!(
                    "<div class=\"Embed\"><div class=\"Embed__ResponsiveContainer\"><iframe src=\"{}\" style=\"border: none;\"></iframe></div></div>",
                    url
                )
            }
            Block::Bookmark { bookmark } => {
                let url = bookmark.url.clone();
                let caption = Self::render_rich_text(&bookmark.caption);
                
                let parts: Vec<&str> = if caption.contains('|') {
                    caption.splitn(2, '|').collect()
                } else if caption.contains('\n') {
                    caption.splitn(2, '\n').collect()
                } else {
                    vec![&caption, ""]
                };

                let mut title = parts[0].trim().to_string();
                let desc = parts.get(1).map(|s| s.trim()).unwrap_or("");
                
                let display_url = url.trim_start_matches("https://").trim_start_matches("http://").trim_end_matches("/");
                let host = display_url.split('/').next().unwrap_or(display_url);

                // 如果没有标题（caption 为空），直接显示原始 URL
                if title.is_empty() {
                    title = url.clone();
                }
                
                let icon_url = format!("https://www.google.com/s2/favicons?domain={}&sz=64", host);

                let desc_html = if !desc.is_empty() {
                    format!("<div class=\"Bookmark__Desc\">{}</div>", desc)
                } else {
                    "".to_string()
                };

                format!(
                    "<div class=\"Bookmark\"><a href=\"{}\" target=\"_blank\">
                        <div class=\"Bookmark__Content\">
                            <div class=\"Bookmark__Title\">{}</div>
                            {}
                            <div class=\"Bookmark__Meta\">
                                <img class=\"Bookmark__Icon\" src=\"{}\" />
                                <span class=\"Bookmark__Link\">{}</span>
                            </div>
                        </div>
                    </a></div>",
                    url, title, desc_html, icon_url, display_url
                )
            }
            Block::Toggle { toggle } => {
                let text = Self::render_rich_text(&toggle.rich_text);
                format!(
                    "<details class=\"Toggle\"><summary class=\"Toggle__Summary\">{}</summary>",
                    text
                )
            }
            Block::ToDo { to_do } => {
                let text = Self::render_rich_text(&to_do.rich_text);
                let checked = if to_do.checked { "checked" } else { "" };
                let checked_class = if to_do.checked { "todo-checked" } else { "" };
                format!(
                    "<div class=\"todo-item\">
                        <input type=\"checkbox\" class=\"todo-checkbox\" {} disabled>
                        <span class=\"{}\">{}</span>
                    </div>",
                    checked, checked_class, text
                )
            }
            Block::Equation { equation } => {
                format!(
                    "<div class=\"Equation\"><div class=\"equation-block\">{}</div></div>",
                    equation.expression
                )
            }
            Block::Divider { .. } => "<div class=\"Divider\"></div>".to_string(),
            Block::ColumnList { .. } => "<div class=\"ColumnList\">".to_string(), // 容器开始
            Block::Column { .. } => "<div class=\"Column\" style=\"flex: 1; min-width: 0;\">".to_string(), // 容器开始
            Block::Table { table } => {
                format!(
                    "<div class=\"Table\"><table style=\"width: {}px\">",
                    table.table_width as i32
                )
            }
            Block::TableRow { table_row } => {
                let mut row_html = String::from("<tr>");
                for cell_rich_text in &table_row.cells {
                    row_html.push_str("<td>");
                    row_html.push_str(&Self::render_rich_text(cell_rich_text));
                    row_html.push_str("</td>");
                }
                row_html.push_str("</tr>");
                row_html
            }
            _ => format!("<!-- Unsupported block type: {:?} -->", block),
        }
    }

    pub fn render_rich_text(rich_texts: &[RichText]) -> String {
        let mut html = String::new();
        for rt in rich_texts {
            match rt {
                RichText::Text {
                    text, annotations, ..
                } => {
                    let mut content = text.content.clone();

                    if annotations.bold {
                        content = format!("<strong>{}</strong>", content);
                    }
                    if annotations.italic {
                        content = format!("<em>{}</em>", content);
                    }
                    if annotations.strikethrough {
                        content = format!("<del>{}</del>", content);
                    }
                    if annotations.underline {
                        content = format!("<u>{}</u>", content);
                    }
                    if annotations.code {
                        content = format!("<code>{}</code>", content);
                    }

                    // Handle Color
                    let color_part = Self::get_color_part(&annotations.color);
                    if !color_part.is_empty() {
                        let color_class = format!("SemanticString__Fragment--{}", color_part);
                        content = format!("<span class=\"{}\">{}</span>", color_class, content);
                    }

                    html.push_str(&content);
                }
                RichText::Equation { equation, .. } => {
                    html.push_str(&format!(
                        "<span class=\"equation-inline\">{}</span>",
                        equation.expression
                    ));
                }
                _ => {} // Handle mentions if needed
            }
        }
        html
    }

    fn get_color_part(color: &Color) -> String {
        let mut color_str = format!("{:?}", color);

        // 如果是 Option 包装（通常是因为 Debug 格式显示 Some(Red)）
        if color_str.starts_with("Some(") {
            color_str = color_str
                .strip_prefix("Some(")
                .unwrap()
                .strip_suffix(")")
                .unwrap()
                .to_string();
        }

        if color_str == "Default" {
            return String::new();
        }

        // Notion variants are PascalCase in notionrs: Red, RedBackground, etc.
        // theme.css expects: ColorRed, BgRed
        if color_str.ends_with("Background") {
            let base = color_str.strip_suffix("Background").unwrap();
            format!("Bg{}", base)
        } else {
            format!("Color{}", color_str)
        }
    }
}
