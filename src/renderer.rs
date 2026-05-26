use notionrs_types::prelude::*;

pub struct HtmlRenderer;

#[derive(Debug, Clone)]
pub struct TocEntry {
    pub level: u8,
    pub text: String,
    pub anchor_id: String,
}

impl HtmlRenderer {
    pub fn render_block(block: &Block, block_id: &str) -> String {
        match block {
            Block::Paragraph { paragraph } => {
                let text = Self::render_rich_text(&paragraph.rich_text);
                let color_part = Self::get_color_part(&paragraph.color);
                let color_class = if color_part.is_empty() {
                    String::new()
                } else {
                    format!("ColorfulBlock--{}", color_part)
                };
                if let Some(icon) = &paragraph.icon {
                    let icon_html = Self::render_emoji_and_icon(icon);
                    format!(
                        "<div class=\"Paragraph Paragraph--WithIcon {}\"><div class=\"Paragraph__Icon\">{}</div><p class=\"Paragraph__Text\">{}</p></div>",
                        color_class, icon_html, text
                    )
                } else {
                    format!("<p class=\"Paragraph {}\">{}</p>", color_class, text)
                }
            }
            Block::Heading1 { heading_1 } => {
                let text = Self::render_rich_text(&heading_1.rich_text);
                let color_part = Self::get_color_part(&heading_1.color);
                Self::render_heading(1, &text, &color_part, block_id)
            }
            Block::Heading2 { heading_2 } => {
                let text = Self::render_rich_text(&heading_2.rich_text);
                let color_part = Self::get_color_part(&heading_2.color);
                Self::render_heading(2, &text, &color_part, block_id)
            }
            Block::Heading3 { heading_3 } => {
                let text = Self::render_rich_text(&heading_3.rich_text);
                let color_part = Self::get_color_part(&heading_3.color);
                Self::render_heading(3, &text, &color_part, block_id)
            }
            Block::Heading4 { heading_4 } => {
                let text = Self::render_rich_text(&heading_4.rich_text);
                let color_part = Self::get_color_part(&heading_4.color);
                Self::render_heading(4, &text, &color_part, block_id)
            }
            Block::BulletedListItem { bulleted_list_item } => {
                let text = Self::render_rich_text(&bulleted_list_item.rich_text);
                let color_part = Self::get_color_part(&bulleted_list_item.color);
                let color_class = if color_part.is_empty() {
                    String::new()
                } else {
                    format!("ColorfulBlock--{}", color_part)
                };
                format!("<li class=\"BulletedList {}\">{}</li>", color_class, text)
            }
            Block::NumberedListItem { numbered_list_item } => {
                let text = Self::render_rich_text(&numbered_list_item.rich_text);
                let color_part = Self::get_color_part(&numbered_list_item.color);
                let color_class = if color_part.is_empty() {
                    String::new()
                } else {
                    format!("ColorfulBlock--{}", color_part)
                };
                format!("<li class=\"NumberedList {}\">{}</li>", color_class, text)
            }
            Block::Code { code } => {
                let language = format!("{:?}", code.language).to_lowercase();
                let text = Self::render_rich_text(&code.rich_text);
                format!(
                    "<pre class=\"Code\"><code class=\"language-{}\">{}</code></pre>",
                    language, text
                )
            }
            Block::Quote { quote } => {
                let text = Self::render_rich_text(&quote.rich_text);
                let color_part = Self::get_color_part(&quote.color);
                let color_class = if color_part.is_empty() {
                    String::new()
                } else {
                    format!("ColorfulBlock--{}", color_part)
                };
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
                let color_class = if color_part.is_empty() {
                    String::new()
                } else {
                    format!("ColorfulBlock--{}", color_part)
                };
                format!(
                    "<div class=\"Callout {}\"><div class=\"Callout__Icon\">{}</div><div class=\"Callout__Content\">{}</div></div>",
                    color_class, emoji, text
                )
            }
            Block::Image { image } => {
                let (url, caption) = match image {
                    File::External(ext) => (ext.external.url.clone(), ext.caption.clone()),
                    File::NotionHosted(int) => (int.file.url.clone(), int.caption.clone()),
                    _ => (String::new(), None),
                };
                let caption_vec = caption.unwrap_or_default();
                let caption_str = Self::render_rich_text(&caption_vec);
                let caption_html = if caption_str.is_empty() {
                    String::new()
                } else {
                    format!("<figcaption>{}</figcaption>", caption_str)
                };

                format!(
                    "<div class=\"Image Image--Normal\"><figure><img src=\"{}\" alt=\"{}\" loading=\"lazy\" />{}</figure></div>",
                    url,
                    caption_str.replace("\"", "&quot;"),
                    caption_html
                )
            }
            Block::Video { video } => {
                let url = match video {
                    File::External(ext) => ext.external.url.clone(),
                    File::NotionHosted(int) => int.file.url.clone(),
                    _ => video.to_string(),
                };
                format!(
                    "<div class=\"Video\"><div class=\"Video__Content\"><video controls src=\"{}\"></video></div></div>",
                    url
                )
            }
            Block::Audio { audio } => {
                let url = match audio {
                    File::External(ext) => ext.external.url.clone(),
                    File::NotionHosted(int) => int.file.url.clone(),
                    _ => audio.to_string(),
                };
                format!(
                    "<div class=\"Audio\"><audio controls src=\"{}\"></audio></div>",
                    url
                )
            }
            Block::File { file } => {
                let url = match file {
                    File::External(ext) => ext.external.url.clone(),
                    File::NotionHosted(int) => int.file.url.clone(),
                    _ => file.to_string(),
                };
                // 仅提取文件名，避免暴露包含加密签名的长 URL
                let name = url.split('?').next().unwrap_or(&url); // 去掉查询参数
                let name = name.split('/').last().unwrap_or("Download File");
                format!(
                    "<div class=\"File\"><a href=\"{}\" target=\"_blank\"><div class=\"File__Icon\">📎</div><div class=\"File__Title\">{}</div></a></div>",
                    url, name
                )
            }
            Block::Pdf { pdf } => {
                let url = match pdf {
                    File::External(ext) => ext.external.url.clone(),
                    File::NotionHosted(int) => int.file.url.clone(),
                    _ => pdf.to_string(),
                };
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

                // 支持 "标题 | 描述 | 缩略图URL" 的格式
                let (mut title, desc, thumb_url) = {
                    let parts: Vec<&str> = caption.split('|').map(|s| s.trim()).collect();
                    match parts.len() {
                        len if len >= 3 => (
                            parts[0].to_string(),
                            parts[1].to_string(),
                            parts[2].to_string(),
                        ),
                        2 => (parts[0].to_string(), parts[1].to_string(), String::new()),
                        1 if !parts[0].is_empty() => {
                            (parts[0].to_string(), String::new(), String::new())
                        }
                        _ => (url.clone(), String::new(), String::new()),
                    }
                };

                let display_url = url
                    .trim_start_matches("https://")
                    .trim_start_matches("http://")
                    .trim_end_matches("/");
                let host = display_url.split('/').next().unwrap_or(display_url);

                if title.is_empty() {
                    title = url.clone();
                }

                let desc_html = if !desc.is_empty() {
                    format!("<div class=\"Bookmark__Desc\">{}</div>", desc)
                } else {
                    "".to_string()
                };

                let thumb_html = if !thumb_url.is_empty() && thumb_url.starts_with("http") {
                    format!(
                        "<div class=\"Bookmark__Thumbnail\"><img src=\"{}\" alt=\"{} preview\" loading=\"lazy\" /></div>",
                        thumb_url, title
                    )
                } else {
                    "".to_string()
                };

                let icon_url = format!("https://www.google.com/s2/favicons?domain={}&sz=64", host);

                format!(
                    "<div class=\"Bookmark\"><a href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\">
                        <div class=\"Bookmark__Content\">
                            <div class=\"Bookmark__Info\">
                                <div class=\"Bookmark__Title\">{}</div>
                                {}
                                <div class=\"Bookmark__Meta\">
                                    <img class=\"Bookmark__Icon\" src=\"{}\" alt=\"{} favicon\" />
                                    <span class=\"Bookmark__Link\">{}</span>
                                </div>
                            </div>
                            {}
                        </div>
                    </a></div>",
                    url, title, desc_html, icon_url, host, display_url, thumb_html
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
            Block::Column { column } => {
                let ratio = column.width_ratio;
                let flex_basis = ratio * 100.0;
                format!(
                    "<div class=\"Column\" style=\"flex: {}; width: {}%; min-width: 0;\">",
                    ratio, flex_basis
                )
            }
            Block::Table { .. } => {
                "<div class=\"Table\"><table class=\"Table__Simple\" style=\"width: 100%\">"
                    .to_string()
            }
            Block::TableRow { table_row } => {
                let mut row_html = String::from("<tr>");
                for cell_rich_text in &table_row.cells {
                    row_html.push_str("<td class=\"Table__Cell\">");
                    row_html.push_str(&Self::render_rich_text(cell_rich_text));
                    row_html.push_str("</td>");
                }
                row_html.push_str("</tr>");
                row_html
            }
            Block::ChildPage { child_page } => {
                let id_simple = block_id.replace("-", "");
                format!(
                    "<a class=\"Page\" href=\"https://notion.so/{}\" target=\"_blank\"><div><div class=\"Page__Icon\">📄</div><div class=\"Page__Title\"><span class=\"SemanticString\">{}</span></div></div></a>",
                    id_simple, child_page.title
                )
            }
            Block::ChildDatabase { child_database } => {
                // 这里我们不再仅仅显示一个链接，而是预留一个类名，由 main.rs 完成内容填充
                format!(
                    "<div class=\"Database\"><h3 class=\"Database__Title\">{}</h3><div class=\"Database__TablePlaceholder\" data-db-id=\"{}\"></div></div>",
                    child_database.title, block_id
                )
            }
            Block::TableOfContents { .. } => "<div class=\"TableOfContents\"></div>".to_string(),
            Block::Breadcrumb { .. } => "<div class=\"Breadcrumb\"></div>".to_string(),
            Block::LinkPreview { link_preview } => {
                let url = link_preview.url.clone();
                format!(
                    "<div class=\"LinkPreview\"><a href=\"{}\" target=\"_blank\">{}</a></div>",
                    url, url
                )
            }
            /*
            Block::LinkToPage { link_to_page } => {
                let link_id = match &link_to_page.type_ {
                    LinkToPageType::PageId(id) => id.clone(),
                    LinkToPageType::DatabaseId(id) => id.clone(),
                    _ => String::new(),
                };
                if link_id.is_empty() {
                    String::new()
                } else {
                     let id_simple = link_id.replace("-", "");
                     format!(
                        "<a class=\"Page\" href=\"https://notion.so/{}\" target=\"_blank\"><div><div class=\"Page__Icon\">↗️</div><div class=\"Page__Title\"><span class=\"SemanticString\">Link to Page</span></div></div></a>",
                        id_simple
                    )
                }
            }
            */
            Block::SyncedBlock { .. } => {
                // SyncedBlock is a container, its content will be rendered by the recursion in main.rs if handled there.
                // Here we just render a wrapper or nothing.
                "<div class=\"SyncedBlock\">".to_string()
            }
            Block::Template { .. } => String::new(), // Templates are not rendered
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
                    let mut content = Self::escape_html(&text.content);

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

    fn render_emoji_and_icon(icon: &EmojiAndIcon) -> String {
        match icon {
            EmojiAndIcon::Emoji(emoji) => {
                format!(
                    "<span class=\"inline-img-icon\">{}</span>",
                    Self::escape_html(&emoji.emoji)
                )
            }
            EmojiAndIcon::CustomEmoji(custom) => {
                format!(
                    "<img class=\"inline-img-icon\" src=\"{}\" alt=\"{}\" loading=\"lazy\" />",
                    Self::escape_html(&custom.custom_emoji.url),
                    Self::escape_html(&custom.custom_emoji.name)
                )
            }
            EmojiAndIcon::File(File::External(ext)) => {
                format!(
                    "<img class=\"inline-img-icon\" src=\"{}\" alt=\"paragraph icon\" loading=\"lazy\" />",
                    Self::escape_html(&ext.external.url)
                )
            }
            EmojiAndIcon::File(File::NotionHosted(file)) => {
                format!(
                    "<img class=\"inline-img-icon\" src=\"{}\" alt=\"paragraph icon\" loading=\"lazy\" />",
                    Self::escape_html(&file.file.url)
                )
            }
            EmojiAndIcon::File(_) => "<span class=\"inline-img-icon\">⬚</span>".to_string(),
            EmojiAndIcon::Icon(icon) => {
                let label = Self::escape_html(&icon.icon.name);
                format!(
                    "<span class=\"inline-img-icon\" title=\"{}\">{}</span>",
                    label, label
                )
            }
        }
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

    pub fn render_database_table(database_json: &serde_json::Value) -> String {
        let mut html = String::from(
            "<div class=\"Table\"><table class=\"Table__Simple\" style=\"width: 100%\">",
        );

        if let Some(results) = database_json.get("results").and_then(|r| r.as_array()) {
            if results.is_empty() {
                return "<p class=\"Paragraph\">No entries found.</p>".to_string();
            }

            // 获取表头（所有页面中出现的属性并集）
            let mut headers = Vec::new();
            if let Some(first_page) = results.first() {
                if let Some(properties) = first_page.get("properties").and_then(|p| p.as_object()) {
                    for key in properties.keys() {
                        headers.push(key.clone());
                    }
                }
            }

            // 渲染表头
            html.push_str("<thead><tr>");
            for header in &headers {
                html.push_str(&format!(
                    "<th class=\"Table__Cell\" style=\"font-weight: bold;\">{}</th>",
                    header
                ));
            }
            html.push_str("</tr></thead><tbody>");

            // 渲染行
            for page in results {
                html.push_str("<tr>");
                if let Some(properties) = page.get("properties").and_then(|p| p.as_object()) {
                    for header in &headers {
                        html.push_str("<td class=\"Table__Cell\">");
                        if let Some(prop) = properties.get(header) {
                            let prop_type = prop.get("type").and_then(|t| t.as_str()).unwrap_or("");
                            match prop_type {
                                "title" | "rich_text" => {
                                    if let Some(texts) =
                                        prop.get(prop_type).and_then(|t| t.as_array())
                                    {
                                        for rt_json in texts {
                                            if let Some(plain_text) =
                                                rt_json.get("plain_text").and_then(|pt| pt.as_str())
                                            {
                                                html.push_str(&Self::escape_html(plain_text));
                                            }
                                        }
                                    }
                                }
                                "multi_select" => {
                                    if let Some(options) =
                                        prop.get("multi_select").and_then(|o| o.as_array())
                                    {
                                        for opt in options {
                                            if let Some(name) =
                                                opt.get("name").and_then(|n| n.as_str())
                                            {
                                                let color = opt
                                                    .get("color")
                                                    .and_then(|c| c.as_str())
                                                    .unwrap_or("gray");
                                                html.push_str(&format!(
                                                    "<span class=\"tag tag-{}\">{}</span> ",
                                                    color, name
                                                ));
                                            }
                                        }
                                    }
                                }
                                "select" => {
                                    if let Some(opt) =
                                        prop.get("select").and_then(|o| o.as_object())
                                    {
                                        if let Some(name) = opt.get("name").and_then(|n| n.as_str())
                                        {
                                            let color = opt
                                                .get("color")
                                                .and_then(|c| c.as_str())
                                                .unwrap_or("gray");
                                            html.push_str(&format!(
                                                "<span class=\"tag tag-{}\">{}</span>",
                                                color, name
                                            ));
                                        }
                                    }
                                }
                                "checkbox" => {
                                    let checked = prop
                                        .get("checkbox")
                                        .and_then(|c| c.as_bool())
                                        .unwrap_or(false);
                                    html.push_str(if checked { "☑️" } else { "☐" });
                                }
                                "url" => {
                                    if let Some(url) = prop.get("url").and_then(|u| u.as_str()) {
                                        html.push_str(&format!(
                                            "<a href=\"{}\" target=\"_blank\">{}</a>",
                                            url, url
                                        ));
                                    }
                                }
                                _ => {
                                    // 其他类型简单显示其字段名
                                    html.push_str(&format!("[{}]", prop_type));
                                }
                            }
                        }
                        html.push_str("</td>");
                    }
                }
                html.push_str("</tr>");
            }
            html.push_str("</tbody>");
        }

        html.push_str("</table></div>");
        html
    }

    fn render_heading(level: u8, text: &str, color_part: &str, block_id: &str) -> String {
        let color_class = if color_part.is_empty() {
            String::new()
        } else {
            format!("ColorfulBlock--{}", color_part)
        };
        let anchor_id = Self::heading_anchor_id(block_id);
        format!(
            "<h{level} id=\"{anchor_id}\" class=\"Heading Heading--{level} {color_class}\"><a class=\"Anchor\" href=\"#{anchor_id}\" aria-label=\"Anchor\">#</a><span class=\"SemanticString\">{text}</span></h{level}>"
        )
    }

    pub fn heading_anchor_id(block_id: &str) -> String {
        format!("heading-{}", block_id.replace('-', ""))
    }

    pub fn render_table_of_contents(entries: &[TocEntry]) -> String {
        if entries.is_empty() {
            return String::new();
        }

        let mut html = String::from(
            "<aside class=\"TableOfContents\"><div class=\"TableOfContents__Header\">Contents</div>",
        );
        for entry in entries {
            html.push_str(&format!(
                "<div class=\"TableOfContents__Item\" data-level=\"{}\"><a href=\"#{}\">{}</a></div>",
                entry.level,
                Self::escape_html(&entry.anchor_id),
                Self::escape_html(&entry.text)
            ));
        }
        html.push_str("</aside>");
        html
    }

    fn escape_html(s: &str) -> String {
        let mut escaped = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '<' => escaped.push_str("&lt;"),
                '>' => escaped.push_str("&gt;"),
                '&' => escaped.push_str("&amp;"),
                '"' => escaped.push_str("&quot;"),
                '\'' => escaped.push_str("&#39;"),
                _ => escaped.push(c),
            }
        }
        escaped
    }
}
