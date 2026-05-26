mod renderer;

use anyhow::{Context, Result};
use futures::TryStreamExt;
use futures::stream::{self, StreamExt};
use notionrs::Client;
use notionrs::PaginateExt;
use notionrs_types::prelude::*;
use renderer::{HtmlRenderer, TocEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

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

fn post_url_for(title: &str, tags: &[Tag]) -> String {
    let category_slug = tags
        .first()
        .map(|tag| tag.slug.as_str())
        .filter(|slug| !slug.is_empty())
        .unwrap_or("uncategorized");

    format!("tag/{}/{}.html", category_slug, slugify(title))
}

fn root_path_for_url(url: &str) -> String {
    let depth = url.matches('/').count();
    if depth == 0 {
        ".".to_string()
    } else {
        std::iter::repeat("..")
            .take(depth)
            .collect::<Vec<_>>()
            .join("/")
    }
}

fn extract_file_url(file: &File) -> Option<String> {
    match file {
        File::External(ext) => Some(ext.external.url.clone()),
        File::NotionHosted(f) => Some(f.file.url.clone()),
        _ => None,
    }
}

fn extract_icon_url(icon: &EmojiAndIcon) -> Option<String> {
    match icon {
        EmojiAndIcon::Emoji(emoji) => Some(emoji.emoji.clone()),
        EmojiAndIcon::File(file) => extract_file_url(file),
        EmojiAndIcon::CustomEmoji(custom) => Some(custom.custom_emoji.url.clone()),
        EmojiAndIcon::Icon(icon) => Some(icon.icon.name.clone()),
    }
}

type GenericPageProperties = HashMap<String, PageProperty>;

async fn get_block_children_all(client: &Client, block_id: &str) -> Result<Vec<BlockResponse>> {
    let crate_result = client
        .get_block_children()
        .block_id(block_id)
        .into_stream()
        .try_collect::<Vec<BlockResponse>>()
        .await;

    match crate_result {
        Ok(blocks) => Ok(blocks),
        Err(err) => {
            eprintln!(
                "Warning: notionrs get_block_children failed for block {}: {}. Skipping this block's children...",
                block_id, err
            );
            Ok(Vec::new())
        }
    }
}

async fn query_child_database_json(
    client: &Client,
    database_or_data_source_id: &str,
) -> Result<serde_json::Value> {
    let data_source_response = match client
        .retrieve_data_source()
        .data_source_id(database_or_data_source_id)
        .send()
        .await
    {
        Ok(data_source) => data_source,
        Err(_) => {
            let database = client
                .retrieve_database()
                .database_id(database_or_data_source_id)
                .send()
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

            let data_source_id = database
                .data_sources
                .first()
                .map(|ds| ds.id.trim().to_string())
                .unwrap_or_else(|| database_or_data_source_id.to_string());

            client
                .retrieve_data_source()
                .data_source_id(&data_source_id)
                .send()
                .await
                .map_err(|e| anyhow::anyhow!(e))?
        }
    };

    let query_response = client
        .query_data_source()
        .typed::<GenericPageProperties>()
        .data_source_id(&data_source_response.id)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    Ok(serde_json::to_value(query_response)?)
}

/// 获取正确的 ID - 如果输入的是数据库 ID，则自动转换为数据源 ID
/// notionrs 库的 query_data_source 方法需要数据源 ID，而不是数据库 ID
/// 此函数会自动检测并转换
async fn get_correct_id(
    client: &Client,
    database_id: &str,
) -> Result<(String, Option<String>, Option<String>)> {
    let database_id_trimmed = database_id.trim();

    if let Ok(response) = client
        .retrieve_database()
        .database_id(database_id_trimmed)
        .send()
        .await
    {
        let icon_url = response.icon.as_ref().and_then(extract_icon_url);
        let cover = response.cover.as_ref().and_then(extract_file_url);
        let id = if !response.data_sources.is_empty() {
            response.data_sources[0].id.trim().to_string()
        } else {
            database_id_trimmed.to_string()
        };
        return Ok((id, icon_url, cover));
    }

    let response = client
        .retrieve_data_source()
        .data_source_id(database_id_trimmed)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    Ok((
        response.id.trim().to_string(),
        response.icon.as_ref().and_then(extract_icon_url),
        response.cover.as_ref().and_then(extract_file_url),
    ))
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

fn env_truthy(key: &str) -> bool {
    std::env::var(key)
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn normalize_notion_id(id: &str) -> String {
    id.trim().replace('-', "").to_lowercase()
}

async fn collect_toc_entries(client: &Client, block_id: &str) -> Result<Vec<TocEntry>> {
    let mut entries = Vec::new();
    let results = get_block_children_all(client, block_id).await?;

    for block_res in results {
        match &block_res.block {
            Block::Heading1 { heading_1 } => entries.push(TocEntry {
                level: 1,
                text: heading_1.to_string(),
                anchor_id: HtmlRenderer::heading_anchor_id(&block_res.id),
            }),
            Block::Heading2 { heading_2 } => entries.push(TocEntry {
                level: 2,
                text: heading_2.to_string(),
                anchor_id: HtmlRenderer::heading_anchor_id(&block_res.id),
            }),
            Block::Heading3 { heading_3 } => entries.push(TocEntry {
                level: 3,
                text: heading_3.to_string(),
                anchor_id: HtmlRenderer::heading_anchor_id(&block_res.id),
            }),
            Block::Heading4 { heading_4 } => entries.push(TocEntry {
                level: 4,
                text: heading_4.to_string(),
                anchor_id: HtmlRenderer::heading_anchor_id(&block_res.id),
            }),
            _ => {}
        }

        let is_database = matches!(block_res.block, Block::ChildDatabase { .. });
        if block_res.has_children && !is_database {
            entries.extend(Box::pin(collect_toc_entries(client, &block_res.id)).await?);
        }
    }

    Ok(entries)
}

async fn print_available_views(client: &Client, data_source_id: &str) {
    let result = client
        .list_views()
        .data_source_id(data_source_id)
        .page_size(100)
        .send()
        .await;

    match result {
        Ok(response) => {
            if response.results.is_empty() {
                println!(">>> 当前 data source 没有可列出的 view");
                return;
            }

            println!(">>> 可用 Views:");
            for view_ref in response.results {
                match client.retrieve_view().view_id(&view_ref.id).send().await {
                    Ok(view) => println!("    - {} [{}] {}", view.name, view.r#type, view.id),
                    Err(err) => eprintln!("!!! 读取 view {} 失败: {}", view_ref.id, err),
                }
            }
        }
        Err(err) => eprintln!("!!! 列出 views 失败: {}", err),
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ViewDirectoryItem {
    id: String,
    name: String,
    view_type: String,
    url: String,
}

async fn list_view_directory_items(
    client: &Client,
    data_source_id: &str,
) -> Result<Vec<ViewDirectoryItem>> {
    let response = client
        .list_views()
        .data_source_id(data_source_id)
        .page_size(100)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    let mut items = Vec::new();
    for view_ref in response.results {
        let view = client
            .retrieve_view()
            .view_id(&view_ref.id)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        items.push(ViewDirectoryItem {
            id: view.id,
            name: view.name.clone(),
            view_type: view.r#type.to_string(),
            url: format!("view/{}.html", slugify(&view.name)),
        });
    }

    Ok(items)
}

async fn render_view_page(
    client: &Client,
    tera: &tera::Tera,
    site_meta: &SiteMeta,
    processed_posts_by_id: &HashMap<String, PostMetadata>,
    all_tags: &[TagStat],
    view_id: &str,
) -> Result<()> {
    let view = client
        .retrieve_view()
        .view_id(view_id)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    let query = client
        .create_view_query()
        .view_id(view_id)
        .page_size(100)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    let results = client
        .get_view_query_results()
        .view_id(view_id)
        .query_id(&query.id)
        .page_size(100)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    let mut pages = Vec::new();
    for page_ref in &results.results {
        if let Some(meta) = processed_posts_by_id.get(&normalize_notion_id(&page_ref.id)) {
            pages.push(meta.clone());
        }
    }
    pages.sort_by(|a, b| b.date.cmp(&a.date));

    fs::create_dir_all("public/view")?;

    let view_site_meta = SiteMeta {
        title: format!("View: {}", view.name),
        icon_url: site_meta.icon_url.clone(),
        cover: site_meta.cover.clone(),
        pages: pages.clone(),
    };

    let mut context = tera::Context::new();
    context.insert("siteMeta", &view_site_meta);
    context.insert("viewName", &view.name);
    context.insert("viewType", &view.r#type.to_string());
    context.insert("pages", &pages);
    context.insert("allTags", all_tags);
    context.insert("rootPath", "..");

    let output_path = format!("public/view/{}.html", slugify(&view.name));
    let html = tera.render("view.html", &context)?;
    fs::write(&output_path, html)?;
    println!(">>> 已生成实验 View 页面: {}", output_path);

    if let Err(err) = client
        .delete_view_query()
        .view_id(view_id)
        .query_id(&query.id)
        .send()
        .await
    {
        eprintln!("!!! 删除 view query 缓存失败: {}", err);
    }

    Ok(())
}

async fn render_view_directory(
    tera: &tera::Tera,
    site_meta: &SiteMeta,
    items: &[ViewDirectoryItem],
) -> Result<()> {
    fs::create_dir_all("public/view")?;
    let mut context = tera::Context::new();
    context.insert("siteMeta", site_meta);
    context.insert("views", items);
    context.insert("rootPath", "..");
    let html = tera.render("viewIndex.html", &context)?;
    fs::write("public/view/index.html", html)?;
    println!(">>> 已生成 View 目录页: public/view/index.html");
    Ok(())
}

async fn get_page_html(
    client: &Client,
    page_id: &str,
    toc_entries: &[TocEntry],
) -> Result<(String, String)> {
    let mut html = String::new();
    let mut plain_text = String::new();

    let mut list_stack: Vec<&str> = Vec::new();
    let results = get_block_children_all(client, page_id).await?;

    for block_res in results {
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
                    let wrapper_class = if current == "ul" {
                        "BulletedListWrapper"
                    } else {
                        "NumberedListWrapper"
                    };
                    html.push_str(&format!("<{} class=\"{}\">\n", current, wrapper_class));
                    list_stack.push(current);
                }
            }
            (None, Some(current)) => {
                // 开启新列表
                let wrapper_class = if current == "ul" {
                    "BulletedListWrapper"
                } else {
                    "NumberedListWrapper"
                };
                html.push_str(&format!("<{} class=\"{}\">\n", current, wrapper_class));
                list_stack.push(current);
            }
            (None, None) => {}
        }

        let block_html_content: String;

        // 处理目录与 ChildDatabase 的内容渲染
        if let Block::TableOfContents { .. } = block {
            block_html_content = HtmlRenderer::render_table_of_contents(toc_entries);
        } else if let Block::ChildDatabase { .. } = block {
            if let Ok(db_json) = query_child_database_json(client, &block_res.id).await {
                block_html_content = HtmlRenderer::render_database_table(&db_json);
            } else {
                block_html_content = HtmlRenderer::render_block(block, &block_res.id);
            }
        } else {
            block_html_content = HtmlRenderer::render_block(block, &block_res.id);
        }

        // 处理容器类 Block (Toggle, ColumnList, Column, Table, SyncedBlock)
        match block {
            Block::Toggle { .. }
            | Block::ColumnList { .. }
            | Block::Column { .. }
            | Block::Table { .. }
            | Block::SyncedBlock { .. } => {
                html.push_str(&block_html_content);
                if block_res.has_children {
                    // 对于 Toggle，开启内容包装层
                    if let Block::Toggle { .. } = block {
                        html.push_str("<div class=\"Toggle__Content\">\n");
                    }

                    let (children_html, children_text) =
                        Box::pin(get_page_html(client, &block_res.id, toc_entries)).await?;
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
                    let (children_html, children_text) =
                        Box::pin(get_page_html(client, &block_res.id, toc_entries)).await?;
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

    // 闭合可能存在的列表
    if let Some(last) = list_stack.pop() {
        html.push_str(&format!("</{}>\n", last));
    }
    Ok((html, plain_text))
}

#[tokio::main]
async fn main() -> Result<()> {
    // 从环境变量读取配置
    let notion_token = std::env::var("NOTION_TOKEN").context("未设置 NOTION_TOKEN 环境变量")?;
    let database_id = std::env::var("DATABASE_ID").context("未设置 DATABASE_ID 环境变量")?;
    let site_title = std::env::var("SITE_TITLE").unwrap_or_else(|_| "My Blog".to_string());
    let site_cover_override = std::env::var("SITE_COVER").ok();
    let print_notion_views = env_truthy("PRINT_NOTION_VIEWS");
    let generate_view_directory = env_truthy("GENERATE_VIEW_DIRECTORY");
    let notion_view_id = std::env::var("NOTION_VIEW_ID")
        .ok()
        .filter(|v| !v.trim().is_empty());

    println!(">>> 配置信息:");
    println!("    Database ID: {}", database_id);
    println!("    Site Title: {}", site_title);
    if let Some(view_id) = &notion_view_id {
        println!("    实验 View ID: {}", view_id);
    }
    if generate_view_directory {
        println!("    生成 View 目录: 开启");
    }

    let client = Arc::new(Client::new(&notion_token));

    // 尝试获取数据库信息以确定正确的 ID 类型
    // 自动获取正确的数据源 ID (以及全站图标/封面)
    let (data_source_id, site_icon, site_cover) = get_correct_id(&client, &database_id).await?;

    if print_notion_views {
        print_available_views(&client, &data_source_id).await;
    }

    // 2. 初始化 Tera 模板引擎
    let mut tera = tera::Tera::new("templates/**/*.html")?;
    tera.full_reload()?;
    let tera = Arc::new(tera);

    // 3. 获取所有文章元数据
    println!(">>> 正在获取文章列表...");
    let filter = Filter::timestamp_is_not_empty();
    let response = client
        .query_data_source()
        .typed::<MyProperties>()
        .data_source_id(&data_source_id)
        .filter(filter)
        .send()
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

        let date_str = p
            .date
            .date
            .as_ref()
            .and_then(|d| d.start.as_ref())
            .map(|dt| dt.to_string())
            .unwrap_or_else(|| "".to_string());

        // 提取页面图标 (Emoji 或 URL)
        let icon_url = page.icon.as_ref().and_then(extract_icon_url);

        // 提取封面图片 URL
        let cover = page.cover.as_ref().and_then(extract_file_url);

        let tags: Vec<Tag> = p
            .tags
            .multi_select
            .iter()
            .map(|opt| {
                let mut color = format!("{:?}", opt.color).to_lowercase();
                if color.starts_with("some(") {
                    color = color
                        .strip_prefix("some(")
                        .unwrap()
                        .strip_suffix(")")
                        .unwrap()
                        .to_string();
                }
                Tag {
                    name: opt.name.clone(),
                    color,
                    slug: slugify(&opt.name),
                }
            })
            .collect();
        let url = post_url_for(&title, &tags);

        all_posts.push((
            page.id.to_string(),
            PostMetadata {
                title,
                url,
                date: date_str,
                tags,
                preview: "".to_string(), // 稍后填充
                publish: p.publish.checkbox,
                in_menu: p.in_menu.checkbox,
                in_list: p.in_list.checkbox,
                icon_url,
                cover,
            },
        ));
    }

    // 按日期降序排序 (最新的在前)
    all_posts.sort_by(|a, b| b.1.date.cmp(&a.1.date));

    let site_meta = SiteMeta {
        title: site_title,
        icon_url: site_icon,
        cover: site_cover_override.or(site_cover),
        pages: all_posts.iter().map(|(_, m)| m.clone()).collect(),
    };

    fs::create_dir_all("public")?;

    // 4. 并发处理每篇文章
    let concurrency_limit = 5;
    let mut posts_stream = stream::iter(all_posts)
        .map(|(page_id, mut meta)| {
            let client = Arc::clone(&client);
            let tera = Arc::clone(&tera);
            let site_meta = site_meta.clone();

            async move {
                if !meta.publish {
                    return Ok::<Option<(String, PostMetadata)>, anyhow::Error>(None);
                }

                println!(">>> 正在处理: {} ({})", meta.title, page_id);
                let toc_entries = collect_toc_entries(&client, &page_id).await?;
                let result = get_page_html(&client, &page_id, &toc_entries).await;
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
                    root_path: root_path_for_url(&meta.url),
                };

                // 根据标题或标签选择模板
                let is_note = meta.title == "Note" || meta.tags.iter().any(|t| t.name == "Note");
                let template_name = if is_note { "note.html" } else { "post.html" };

                let rendered =
                    tera.render(template_name, &tera::Context::from_serialize(&context)?)?;
                let output_path = Path::new("public").join(&meta.url);
                if let Some(parent) = output_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(output_path, rendered)?;

                Ok(Some((page_id, meta)))
            }
        })
        .buffer_unordered(concurrency_limit);

    let mut processed_posts = Vec::new();
    let mut posts_meta_for_index = Vec::new();
    while let Some(res) = posts_stream.next().await {
        if let Ok(Some((page_id, meta))) = res {
            processed_posts.push((page_id.clone(), meta.clone()));
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
            tags_map
                .entry(tag.name.clone())
                .or_insert_with(Vec::new)
                .push(post.clone());
        }
    }

    // 计算标签统计信息
    let mut all_tags: Vec<TagStat> = Vec::new();
    for (tag_name, posts) in &tags_map {
        let color = posts
            .first()
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

    let processed_posts_by_id: HashMap<String, PostMetadata> = processed_posts
        .into_iter()
        .map(|(id, meta)| (normalize_notion_id(&id), meta))
        .collect();

    if generate_view_directory {
        match list_view_directory_items(&client, &data_source_id).await {
            Ok(items) => {
                if let Err(err) = render_view_directory(&tera, &site_meta, &items).await {
                    eprintln!("!!! 生成 View 目录页失败: {}", err);
                }
            }
            Err(err) => eprintln!("!!! 获取 View 目录失败: {}", err),
        }
    }

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

    if let Some(view_id) = notion_view_id {
        if let Err(err) = render_view_page(
            &client,
            &tera,
            &site_meta,
            &processed_posts_by_id,
            &all_tags,
            &view_id,
        )
        .await
        {
            eprintln!("!!! 生成实验 View 页面失败: {}", err);
        }
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
