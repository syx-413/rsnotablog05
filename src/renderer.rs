use notionrs_types::prelude::*;
use anyhow::Result;

pub struct HtmlRenderer;

impl HtmlRenderer {
    pub fn render_block(block: &Block) -> String {
        match block {
            Block::Paragraph { paragraph } => {
                let text = Self::render_rich_text(&paragraph.rich_text);
                let color_class = Self::get_color_class(&paragraph.color);
                format!("<p class=\"{}\">{}</p>", color_class, text)
            }
            Block::Heading1 { heading_1 } => {
                let text = Self::render_rich_text(&heading_1.rich_text);
                let color_class = Self::get_color_class(&heading_1.color);
                format!("<h1 class=\"{}\">{}</h1>", color_class, text)
            }
            Block::Heading2 { heading_2 } => {
                let text = Self::render_rich_text(&heading_2.rich_text);
                let color_class = Self::get_color_class(&heading_2.color);
                format!("<h2 class=\"{}\">{}</h2>", color_class, text)
            }
            Block::Heading3 { heading_3 } => {
                let text = Self::render_rich_text(&heading_3.rich_text);
                let color_class = Self::get_color_class(&heading_3.color);
                format!("<h3 class=\"{}\">{}</h3>", color_class, text)
            }
            Block::BulletedListItem { bulleted_list_item } => {
                let text = Self::render_rich_text(&bulleted_list_item.rich_text);
                let color_class = Self::get_color_class(&bulleted_list_item.color);
                format!("<li class=\"{}\">{}</li>", color_class, text)
            }
            Block::NumberedListItem { numbered_list_item } => {
                let text = Self::render_rich_text(&numbered_list_item.rich_text);
                let color_class = Self::get_color_class(&numbered_list_item.color);
                format!("<li class=\"{}\">{}</li>", color_class, text)
            }
            Block::Code { code } => {
                let text = Self::render_rich_text(&code.rich_text);
                format!("<pre><code class=\"language-{}\">{}</code></pre>", code.language, text)
            }
            Block::Quote { quote } => {
                let text = Self::render_rich_text(&quote.rich_text);
                let color_class = Self::get_color_class(&quote.color);
                format!("<blockquote class=\"{}\">{}</blockquote>", color_class, text)
            }
            Block::Callout { callout } => {
                let text = Self::render_rich_text(&callout.rich_text);
                let emoji = match &callout.icon {
                    Some(icon) => icon.to_string(),
                    None => "💡".to_string(),
                };
                let color_class = Self::get_color_class(&callout.color);
                format!("<div class=\"callout {}\"><span style=\"margin-right: 10px;\">{}</span>{}</div>", color_class, emoji, text)
            }
            Block::Image { image } => {
                let url = image.to_string();
                format!("<figure><img src=\"{}\" style=\"max-width: 100%; border-radius: 5px;\" /><figcaption></figcaption></figure>", url)
            }
            Block::Video { video } => {
                let url = video.to_string();
                format!("<div class=\"video-block\"><video controls src=\"{}\" style=\"max-width: 100%; border-radius: 5px;\"></video></div>", url)
            }
            Block::Audio { audio } => {
                let url = audio.to_string();
                format!("<div class=\"audio-block\"><audio controls src=\"{}\" style=\"width: 100%; margin: 10px 0;\"></audio></div>", url)
            }
            Block::File { file } => {
                let url = file.to_string();
                let name = url.split('/').last().unwrap_or("Download File");
                format!("<div class=\"file-block\"><a href=\"{}\" target=\"_blank\" class=\"file-link\">📎 {}</a></div>", url, name)
            }
            Block::Pdf { pdf } => {
                let url = pdf.to_string();
                format!("<div class=\"pdf-block\"><embed src=\"{}\" type=\"application/pdf\" width=\"100%\" height=\"500px\" /></div>", url)
            }
            Block::Embed { embed } => {
                let url = embed.url.clone();
                // 简单嵌入 iframe，更复杂的需解析 URL (如 Bilibili, YouTube)
                format!("<div class=\"embed-block\"><iframe src=\"{}\" style=\"width: 100%; height: 400px; border: none;\"></iframe></div>", url)
            }
            Block::Bookmark { bookmark } => {
                let url = bookmark.url.clone();
                // 书签样式
                format!(
                    "<a href=\"{}\" class=\"bookmark\" target=\"_blank\" style=\"display: block; border: 1px solid #ddd; padding: 12px; border-radius: 4px; margin: 10px 0; text-decoration: none; color: inherit;\">
                        <div style=\"font-weight: bold;\">{}</div>
                        <div style=\"font-size: 0.9em; color: #666; overflow: hidden; white-space: nowrap; text-overflow: ellipsis;\">{}</div>
                    </a>",
                    url, url, url
                )
            }
            Block::Toggle { toggle } => {
                let text = Self::render_rich_text(&toggle.rich_text);
                // 注意：Toggle 的子内容会在 main.rs 的递归中处理，但这里我们无法直接包裹子内容
                // 因为 main.rs 的逻辑是平铺渲染。
                // *重要*：目前的 main.rs 逻辑对于 Toggle 这种容器类 Block 支持不够完美（它只是简单的平铺）。
                // 为了完美支持 Toggle，需要在 main.rs 中特殊处理容器 Block 的闭合标签。
                // 但作为 renderer 的一部分，我们至少可以渲染 summary。
                format!("<details><summary>{}</summary></details>", text)
            }
            Block::ToDo { to_do } => {
                let text = Self::render_rich_text(&to_do.rich_text);
                let checked = if to_do.checked { "checked" } else { "" };
                let style = if to_do.checked { "text-decoration: line-through; opacity: 0.7;" } else { "" };
                format!(
                    "<div class=\"todo-item\" style=\"display: flex; align-items: center; margin: 4px 0;\">
                        <input type=\"checkbox\" {} disabled style=\"margin-right: 8px;\">
                        <span style=\"{}\">{}</span>
                    </div>",
                    checked, style, text
                )
            }
            Block::Equation { equation } => {
                format!("<div class=\"equation-block\">{}</div>", equation.expression)
            }
            Block::Divider { .. } => "<hr style=\"border: none; border-top: 1px solid #eaeaea; margin: 2em 0;\" />".to_string(),
            _ => format!("<!-- Unsupported block type -->"),
        }
    }

    pub fn render_rich_text(rich_texts: &[RichText]) -> String {
        let mut html = String::new();
        for rt in rich_texts {
            match rt {
                RichText::Text { text, annotations, .. } => {
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
                    let color_class = Self::get_color_class(&annotations.color);
                    if !color_class.is_empty() {
                        content = format!("<span class=\"{}\">{}</span>", color_class, content);
                    }

                    html.push_str(&content);
                }
                RichText::Equation { equation, .. } => {
                    html.push_str(&format!("<span class=\"equation-inline\">{}</span>", equation.expression));
                }
                _ => {} // Handle mentions if needed
            }
        }
        html
    }

    fn get_color_class(color: &Color) -> String {
        let color_str = format!("{:?}", color).to_lowercase();
        if color_str == "default" {
            return String::new();
        }
        
        if color_str.ends_with("background") {
            format!("bg-{}", color_str.replace("background", ""))
        } else {
            format!("color-{}", color_str)
        }
    }
}
