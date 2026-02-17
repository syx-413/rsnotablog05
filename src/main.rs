mod renderer;

use anyhow::{Context, Result};
use notionrs::Client;
use notionrs_types::prelude::*;
use renderer::HtmlRenderer;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::collections::HashMap;
use std::sync::Arc;
use futures::stream::{self, StreamExt};

/// 递归拷贝目录
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

// -----------------------------------------------------------
// 渲染上下文
// -----------------------------------------------------------
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SiteMeta {
    title: String,
    icon_url: Option<String>,
    cover: Option<String>,
    pages: Vec<PostMetadata>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PageContext {
    site_meta: SiteMeta,
    post: PostMetadataWithContent,
    root_path: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PostMetadataWithContent {
    title: String,
    content: String,
    date: String,
    tags: Vec<Tag>,
    cover: Option<String>,
    icon_url: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PostMetadata {
    title: String,
    url: String,
    date: String,
    tags: Vec<Tag>,
    preview: String,
    publish: bool,
    in_menu: bool,
    in_list: bool,
    icon_url: Option<String>,
    cover: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Tag {
    name: String,
    color: String,
    slug: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TagStat {
    name: String,
    slug: String,
    count: usize,
    color: String,
}

fn slugify(s: &str) -> String {
    s.trim()
        .replace(' ', "-")
        .replace('/', "-")
        .replace('?', "")
        .replace(':', "")
        .replace('*', "")
        .replace('"', "")
        .replace('<', "")
        .replace('>', "")
        .replace('|', "")
        .to_lowercase()
}

fn extract_file_url(file: &File) -> Option<String> {
    match file {
        File::External(ext) => Some(ext.external.url.clone()),
        File::NotionHosted(f) => Some(f.file.url.clone()),
        _ => None,
    }
}

/// 获取正确的 ID - 如果输入的是数据库 ID，则自动转换为数据源 ID
/// notionrs 库的 query_data_source 方法需要数据源 ID，而不是数据库 ID
/// 此函数会自动检测并转换
async fn get_correct_id(client: &Client, database_id: &str) -> Result<(String, Option<String>, Option<String>)> {
    let database_id_trimmed = database_id.trim();
    
    // 首先尝试使用库自带的方法
    let lib_res = client
        .retrieve_database()
        .database_id(database_id_trimmed)
        .send()
        .await;

    match lib_res {
        Ok(response) => {
            let icon_url = match &response.icon {
                Some(Icon::Emoji(emoji)) => Some(emoji.emoji.clone()),
                Some(Icon::File(file_enum)) => extract_file_url(file_enum),
                Some(Icon::CustomEmoji(custom)) => Some(custom.custom_emoji.url.clone()),
                None => None,
            };
            let cover = response.cover.as_ref().and_then(extract_file_url);
            let id = if !response.data_sources.is_empty() {
                response.data_sources[0].id.trim().to_string()
            } else {
                database_id_trimmed.to_string()
            };
            Ok((id, icon_url, cover))
        }
        Err(e) => {
            eprintln!("Warning: Library retrieve_database failed: {}. Attempting manual fetch...", e);
            
            // 手动回退：使用 reqwest 直接获取，以防库的处理逻辑有问题
            let token = std::env::var("NOTION_TOKEN")?;
            let url = format!("https://api.notion.com/v1/databases/{}", database_id_trimmed);
            let http_client = reqwest::Client::new();
            let resp = http_client.get(&url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Notion-Version", "2022-06-28")
                .send()
                .await?;

            if resp.status().is_success() {
                let json: serde_json::Value = resp.json().await?;
                
                // 简单手动解析 JSON
                let icon_url = if let Some(icon) = json.get("icon") {
                    if let Some(emoji) = icon.get("emoji").and_then(|e| e.as_str()) {
                        Some(emoji.to_string())
                    } else if let Some(file) = icon.get("file").and_then(|f| f.get("url")).and_then(|u| u.as_str()) {
                        Some(file.to_string())
                    } else if let Some(ext) = icon.get("external").and_then(|e| e.get("url")).and_then(|u| u.as_str()) {
                        Some(ext.to_string())
                    } else {
                        None
                    }
                } else {
                    None
                };

                let cover = if let Some(cover_obj) = json.get("cover") {
                    if let Some(url) = cover_obj.get("external").and_then(|e| e.get("url")).and_then(|u| u.as_str()) {
                        Some(url.to_string())
                    } else if let Some(url) = cover_obj.get("file").and_then(|f| f.get("url")).and_then(|u| u.as_str()) {
                        Some(url.to_string())
                    } else {
                        None
                    }
                } else {
                    None
                };

                let id = json.get("data_sources")
                    .and_then(|ds| ds.as_array())
                    .and_then(|arr| arr.get(0))
                    .and_then(|first| first.get("id"))
                    .and_then(|id_val| id_val.as_str())
                    .unwrap_or(database_id_trimmed)
                    .to_string();

                Ok((id, icon_url, cover))
            } else {
                eprintln!("Manual fetch also failed with status: {}", resp.status());
                Ok((database_id_trimmed.to_string(), None, None))
            }
        }
    }
}



// -----------------------------------------------------------
// 数据结构 (Notion API 响应映射)
// -----------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MyProperties {
    #[serde(rename = "title")]
    pub title: PageTitleProperty,

    #[serde(rename = "tags")]
    pub tags: PageMultiSelectProperty,

    #[serde(rename = "template")]
    pub template: PageSelectProperty,

    #[serde(rename = "publish")]
    pub publish: PageCheckboxProperty,

    #[serde(rename = "inMenu")]
    pub in_menu: PageCheckboxProperty,

    #[serde(rename = "inList")]
    pub in_list: PageCheckboxProperty,

    #[serde(rename = "date")]
    pub date: PageDateProperty,
}

async fn get_page_html(client: &Client, notion_token: &str, page_id: &str) -> Result<(String, String)> {
    let mut html = String::new();
    let mut plain_text = String::new();
    
    // 使用 reqwest 直接获取原始 JSON，以绕过库的强制反序列化报错
    let url = format!("https://api.notion.com/v1/blocks/{}/children", page_id);
    let http_client = reqwest::Client::new();
    let response_json: serde_json::Value = http_client
        .get(&url)
        .header("Authorization", format!("Bearer {}", notion_token))
        .header("Notion-Version", "2022-06-28")
        .send()
        .await?
        .json()
        .await?;

    let mut list_stack: Vec<&str> = Vec::new();

    if let Some(results) = response_json.get("results").and_then(|r| r.as_array()) {
        for result_value in results {
            let mut result_value = result_value.clone();

            // 为 Table 和 Column 块注入可能缺失的强制字段 (width_ratio, table_width)
            let block_type = result_value.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if block_type == "table" {
                if let Some(table) = result_value.get_mut("table").and_then(|t| t.as_object_mut()) {
                    table.entry("width_ratio").or_insert(serde_json::json!(1.0));
                    table.entry("table_width").or_insert(serde_json::json!(0));
                }
            } else if block_type == "column" {
                if let Some(column) = result_value.get_mut("column").and_then(|t| t.as_object_mut()) {
                    column.entry("width_ratio").or_insert(serde_json::json!(1.0));
                }
            }

            // 尝试逐个反序列化 Block。如果失败，我们跳过这个 Block 而不是报错整个页面。
            let block_res: BlockResponse = match serde_json::from_value(result_value.clone()) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("!!! 无法解析 block (ID: {}). 错误: {}. 跳过该 Block。", 
                        result_value.get("id").and_then(|v| v.as_str()).unwrap_or("unknown"), 
                        e);
                    continue;
                }
            };
            
            let block = &block_res.block;

            // 处理列表分组
            let current_list_type = match block {
                Block::BulletedListItem { .. } => Some("ul"),
                Block::NumberedListItem { .. } => Some("ol"),
                _ => None,
            };

            match (list_stack.last().cloned(), current_list_type) {
                (Some(last), Some(current)) if last == current => {
                    // 继续同类型列表
                }
                (Some(last), _) => {
                    // 关闭之前的列表
                    html.push_str(&format!("</{}>\n", last));
                    list_stack.pop();
                    // 如果当前也是列表，则开启新列表
                    if let Some(current) = current_list_type {
                        let wrapper_class = if current == "ul" { "BulletedListWrapper" } else { "NumberedListWrapper" };
                        html.push_str(&format!("<{} class=\"{}\">\n", current, wrapper_class));
                        list_stack.push(current);
                    }
                }
                (None, Some(current)) => {
                    // 开启新列表
                    let wrapper_class = if current == "ul" { "BulletedListWrapper" } else { "NumberedListWrapper" };
                    html.push_str(&format!("<{} class=\"{}\">\n", current, wrapper_class));
                    list_stack.push(current);
                }
                (None, None) => {}
            }

            let block_html_content: String;

            // 处理 ChildDatabase 的内容渲染
            if let Block::ChildDatabase { .. } = block {
                // 查询该数据库的所有页面
                let database_query_url = format!("https://api.notion.com/v1/databases/{}/query", block_res.id);
                let resp = reqwest::Client::new()
                    .post(&database_query_url)
                    .header("Authorization", format!("Bearer {}", notion_token))
                    .header("Notion-Version", "2022-06-28")
                    .header("Content-Type", "application/json")
                    .send()
                    .await;

                if let Ok(r) = resp {
                    if let Ok(db_json) = r.json::<serde_json::Value>().await {
                        block_html_content = HtmlRenderer::render_database_table(&db_json);
                    } else {
                        block_html_content = HtmlRenderer::render_block(block, &block_res.id);
                    }
                } else {
                    block_html_content = HtmlRenderer::render_block(block, &block_res.id);
                }
            } else {
                block_html_content = HtmlRenderer::render_block(block, &block_res.id);
            }

            // 处理容器类 Block (Toggle, ColumnList, Column, Table, SyncedBlock)
            match block {
                Block::Toggle { .. } | Block::ColumnList { .. } | Block::Column { .. } | Block::Table { .. } | Block::SyncedBlock { .. } => {
                    html.push_str(&block_html_content);
                    if block_res.has_children {
                        // 对于 Toggle，开启内容包装层
                        if let Block::Toggle { .. } = block {
                            html.push_str("<div class=\"Toggle__Content\">\n");
                        }
                        
                        let (children_html, children_text) = Box::pin(get_page_html(client, notion_token, &block_res.id)).await?;
                        html.push_str(&children_html);
                        
                        if let Block::Toggle { .. } = block {
                            html.push_str("</div>\n");
                        }
                        
                        if plain_text.len() < 200 {
                            plain_text.push_str(&children_text);
                        }
                    }
                    match block {
                        Block::Toggle { .. } => html.push_str("</details>\n"),
                        Block::ColumnList { .. } => html.push_str("</div>\n"),
                        Block::Column { .. } => html.push_str("</div>\n"),
                        Block::Table { .. } => html.push_str("</table></div>\n"),
                        Block::SyncedBlock { .. } => html.push_str("</div>\n"),
                        _ => {}
                    }
                }
                _ => {
                    html.push_str(&block_html_content);
                    if !block_html_content.trim().is_empty() {
                        plain_text.push_str(&block.to_string());
                        plain_text.push(' ');
                    }

                    // 关键修复：ChildDatabase 虽然可能有子节点（Notion 内部视图），
                    // 但这些视图在公开 API 中往往无法正常渲染，会导致“黑框”错误。
                    // 此处我们排除 ChildDatabase，不允许其递归渲染子节点。
                    let is_database = matches!(block, Block::ChildDatabase { .. });

                    if block_res.has_children && !is_database {
                        let (children_html, children_text) = Box::pin(get_page_html(client, notion_token, &block_res.id)).await?;
                        // 对于普通的有子节点的 block，保持缩进
                        html.push_str("<div style=\"margin-left: 20px;\">");
                        html.push_str(&children_html);
                        html.push_str("</div>\n");
                        if plain_text.len() < 200 {
                            plain_text.push_str(&children_text);
                        }
                    }
                }
            }
        }
    }

    // 闭合可能存在的列表
    if let Some(last) = list_stack.pop() {
        html.push_str(&format!("</{}>\n", last));
    }
    Ok((html, plain_text))
}

#[tokio::main]
async fn main() -> Result<()> {
    // 从环境变量读取配置
    let notion_token = std::env::var("NOTION_TOKEN")
        .context("未设置 NOTION_TOKEN 环境变量")?;
    let database_id = std::env::var("DATABASE_ID")
        .context("未设置 DATABASE_ID 环境变量")?;
    let site_title = std::env::var("SITE_TITLE")
        .unwrap_or_else(|_| "My Blog".to_string());

    println!(">>> 配置信息:");
    println!("    Database ID: {}", database_id);
    println!("    Site Title: {}", site_title);

    let client = Arc::new(Client::new(&notion_token));

    // 尝试获取数据库信息以确定正确的 ID 类型
    // 自动获取正确的数据源 ID (以及全站图标/封面)
    let (data_source_id, site_icon, site_cover) = get_correct_id(&client, &database_id).await?;

    // 2. 初始化 Tera 模板引擎
    let mut tera = tera::Tera::new("templates/**/*.html")?;
    tera.full_reload()?;
    let tera = Arc::new(tera);

    // 3. 获取所有文章元数据
    println!(">>> 正在获取文章列表...");
    let filter = Filter::timestamp_is_not_empty();
    let response = client
        .query_data_source()
        .data_source_id(&data_source_id)
        .filter(filter)
        .send::<MyProperties>()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    let mut all_posts = Vec::new();
    for page in response.results {
        let p = page.properties;
        
        // 如果没有点击 publish，则完全跳过该文章的研究与元数据生成
        if !p.publish.checkbox {
            continue;
        }

        let title = p.title.to_string();
        let filename = format!("{}.html", slugify(&title));
        
        let date_str = p.date.date.as_ref()
            .and_then(|d| d.start.as_ref())
            .map(|dt| dt.to_string())
            .unwrap_or_else(|| "".to_string());

        // 提取页面图标 (Emoji 或 URL)
        let icon_url = match &page.icon {
            Some(Icon::Emoji(emoji)) => Some(emoji.emoji.clone()),
            Some(Icon::File(file_enum)) => extract_file_url(file_enum),
            Some(Icon::CustomEmoji(custom)) => Some(custom.custom_emoji.url.clone()),
            None => None,
        };

        // 提取封面图片 URL
        let cover = page.cover.as_ref().and_then(extract_file_url);

        all_posts.push((page.id.to_string(), PostMetadata {
            title,
            url: filename,
            date: date_str,
            tags: p.tags.multi_select.iter().map(|opt| {
                let mut color = format!("{:?}", opt.color).to_lowercase();
                if color.starts_with("some(") {
                    color = color.strip_prefix("some(").unwrap().strip_suffix(")").unwrap().to_string();
                }
                Tag { 
                    name: opt.name.clone(), 
                    color,
                    slug: slugify(&opt.name)
                }
            }).collect(),
            preview: "".to_string(), // 稍后填充
            publish: p.publish.checkbox,
            in_menu: p.in_menu.checkbox,
            in_list: p.in_list.checkbox,
            icon_url,
            cover,
        }));
    }
    
    // 按日期降序排序 (最新的在前)
    all_posts.sort_by(|a, b| b.1.date.cmp(&a.1.date));

    let site_meta = SiteMeta {
        title: site_title,
        icon_url: site_icon,
        cover: site_cover,
        pages: all_posts.iter().map(|(_, m)| m.clone()).collect(),
    };

    fs::create_dir_all("public")?;

    // 4. 并发处理每篇文章
    let concurrency_limit = 5;
    let mut posts_stream = stream::iter(all_posts)
        .map(|(page_id, mut meta)| {
            let client = Arc::clone(&client);
            let tera = Arc::clone(&tera);
            let notion_token = notion_token.clone();
            let site_meta = site_meta.clone();

            async move {
                if !meta.publish {
                    return Ok::<Option<PostMetadata>, anyhow::Error>(None);
                }

                println!(">>> 正在处理: {}", meta.title);
                let result = get_page_html(&client, &notion_token, &page_id).await;
                let (content_html, plain_text) = match result {
                    Ok(res) => res,
                    Err(e) => {
                        eprintln!("!!! 处理 '{}' (ID: {}) 失败: {}", meta.title, page_id, e);
                        return Ok(None);
                    }
                };

                let preview = if plain_text.chars().count() > 150 {
                    format!("{}...", plain_text.chars().take(150).collect::<String>())
                } else {
                    plain_text
                };
                meta.preview = preview;

                let post_context = PostMetadataWithContent {
                    title: meta.title.clone(),
                    content: content_html,
                    date: meta.date.clone(),
                    tags: meta.tags.clone(),
                    cover: meta.cover.clone(),
                    icon_url: meta.icon_url.clone(),
                    description: Some(meta.preview.clone()),
                };

                let context = PageContext {
                    site_meta,
                    post: post_context,
                    root_path: ".".to_string(),
                };

                let rendered = tera.render("post.html", &tera::Context::from_serialize(&context)?)?;
                fs::write(format!("public/{}", meta.url), rendered)?;

                Ok(Some(meta))
            }
        })
        .buffer_unordered(concurrency_limit);

    let mut posts_meta_for_index = Vec::new();
    while let Some(res) = posts_stream.next().await {
        if let Ok(Some(meta)) = res {
            if meta.in_list {
                posts_meta_for_index.push(meta);
            }
        }
    }

    // 重新排序，因为并發處理會打亂順序
    posts_meta_for_index.sort_by(|a, b| b.date.cmp(&a.date));

    // 5. 渲染首页
    println!(">>> 正在生成首页...");
    let mut index_context = tera::Context::new();
    index_context.insert("siteMeta", &site_meta);
    index_context.insert("pages", &posts_meta_for_index);
    index_context.insert("rootPath", ".");
    let index_html = tera.render("index.html", &index_context)?;
    fs::write("public/index.html", index_html)?;

    // 6. 生成标签页
    println!(">>> 正在生成标签页...");
    fs::create_dir_all("public/tag")?;
    
    // 按标签分组文章
    let mut tags_map: HashMap<String, Vec<PostMetadata>> = HashMap::new();
    for post in &posts_meta_for_index {
        for tag in &post.tags {
            tags_map.entry(tag.name.clone())
                .or_insert_with(Vec::new)
                .push(post.clone());
        }
    }

    // 计算标签统计信息
    let mut all_tags: Vec<TagStat> = Vec::new();
    for (tag_name, posts) in &tags_map {
        let color = posts.first()
            .and_then(|p| p.tags.iter().find(|t| t.name == *tag_name))
            .map(|t| t.color.clone())
            .unwrap_or_else(|| "default".to_string());
            
        all_tags.push(TagStat {
            name: tag_name.clone(),
            slug: slugify(tag_name),
            count: posts.len(),
            color,
        });
    }
    all_tags.sort_by(|a, b| b.count.cmp(&a.count));

    // 渲染每个标签的页面
    for (tag_name, tag_posts) in tags_map {
        let safe_tag_name = slugify(&tag_name);
        let filename = format!("public/tag/{}.html", safe_tag_name);
        
        let tag_site_meta = SiteMeta {
            title: format!("Tag: {}", tag_name),
            icon_url: None, 
            cover: None,
            pages: tag_posts.clone(),
        };

        let mut context = tera::Context::new();
        context.insert("siteMeta", &tag_site_meta);
        context.insert("tagName", &tag_name);
        context.insert("pages", &tag_posts);
        context.insert("allTags", &all_tags);
        context.insert("rootPath", "..");
        
        let template_name = if tera.get_template_names().any(|t| t == "tag.html") {
            "tag.html"
        } else {
            "index.html"
        };

        let html = tera.render(template_name, &context)?;
        fs::write(filename, html)?;
    }

    // 7. 拷贝静态资源
    if Path::new("templates/main.css").exists() {
        fs::copy("templates/main.css", "public/main.css")?;
    }
    
    let assets_dirs = ["assets", "css", "fonts"];
    for dir in assets_dirs {
        let src = Path::new("templates").join(dir);
        if src.exists() {
            println!(">>> 正在拷贝静态资源: {}...", dir);
            let dst = Path::new("public").join(dir);
            copy_dir_recursive(&src, &dst)?;
        }
    }

    println!(">>> 全部完成！请查看 public/index.html");

    Ok(())
}