mod renderer;

use anyhow::{Context, Result};
use notionrs::Client;
use notionrs_types::prelude::*;
use renderer::HtmlRenderer;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::collections::HashMap;

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
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SiteMeta {
    title: String,
    icon_url: Option<String>,
    pages: Vec<PostMetadata>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PageContext {
    site_meta: SiteMeta,
    post: PostMetadataWithContent,
    root_path: String,
}

#[derive(Debug, Serialize)]
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

/// 获取正确的 ID - 如果输入的是数据库 ID，则自动转换为数据源 ID
/// notionrs 库的 query_data_source 方法需要数据源 ID，而不是数据库 ID
/// 此函数会自动检测并转换
async fn get_correct_id(client: &Client, database_id: &str) -> Result<String> {
    // 使用 retrieve_database 方法获取数据库信息，从中提取数据源 ID
    match client
        .retrieve_database()
        .database_id(database_id.trim())  // 去除首尾空白字符
        .send()
        .await
    {
        Ok(response) => {
            // 从响应中获取数据源 ID
            if !response.data_sources.is_empty() {
                // 返回第一个数据源的 ID，去除首尾空白字符
                Ok(response.data_sources[0].id.trim().to_string())
            } else {
                // 如果没有数据源，返回原始 ID
                Ok(database_id.trim().to_string())
            }
        }
        Err(_) => {
            // 如果 retrieve_database 失败，可能是输入的就是数据源 ID 或权限不足
            // 返回原始 ID，去除首尾空白字符
            Ok(database_id.trim().to_string())
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

async fn get_page_html(client: &Client, page_id: &str) -> Result<(String, String)> {
    let mut html = String::new();
    let mut plain_text = String::new();
    let response = client
        .get_block_children()
        .block_id(page_id)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    let mut list_stack: Vec<&str> = Vec::new();

    let mut i = 0;
    while i < response.results.len() {
        let block_res = &response.results[i];
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

        let block_html = HtmlRenderer::render_block(block);

        // 处理容器类 Block (Toggle, ColumnList, Column, Table)
        match block {
            Block::Toggle { .. } | Block::ColumnList { .. } | Block::Column { .. } | Block::Table { .. } => {
                html.push_str(&block_html);
                if block_res.has_children {
                    // 对于 Toggle，开启内容包装层
                    if let Block::Toggle { .. } = block {
                        html.push_str("<div class=\"Toggle__Content\">\n");
                    }
                    
                    let (children_html, children_text) = Box::pin(get_page_html(client, &block_res.id)).await?;
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
                    _ => {}
                }
            }
            _ => {
                html.push_str(&block_html);
                html.push('\n');

                if plain_text.len() < 200 {
                    plain_text.push_str(&block.to_string());
                    plain_text.push(' ');
                }

                if block_res.has_children {
                    let (children_html, children_text) = Box::pin(get_page_html(client, &block_res.id)).await?;
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
        i += 1;
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

    let client = Client::new(&notion_token);

    // 尝试获取数据库信息以确定正确的 ID 类型
    // 自动获取正确的数据源 ID（如果提供的是数据库 ID）
    // 注意：如果 retrieve_database 调用失败，可能需要直接使用数据源 ID
    let data_source_id = get_correct_id(&client, &database_id).await?;
    // println!(">>> 使用数据源 ID: {}", data_source_id);

    // 2. 初始化 Tera 模板引擎
    let mut tera = tera::Tera::new("templates/**/*.html")?;
    tera.full_reload()?;

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
            Some(Icon::File(file_enum)) => match file_enum {
                File::External(ext_file) => Some(ext_file.external.url.clone()), 
                _ => None, 
            },
            Some(Icon::CustomEmoji(custom)) => Some(custom.custom_emoji.url.clone()),
            None => None,
        };

        // 提取封面图片 URL
        let cover = page.cover.as_ref().map(|c| c.to_string());

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

    let site_meta = SiteMeta {
        title: site_title,
        icon_url: None,
        pages: all_posts.iter().map(|(_, m)| m.clone()).collect(),
    };

    fs::create_dir_all("public")?;

    // 4. 遍历处理每篇文章
    let mut posts_meta_for_index = Vec::new();
    for (page_id, mut meta) in all_posts {
        // 由于上面已经通过 if !p.publish.checkbox 过滤了，这里 meta.publish 理论上全是 true
        // 但保留这个判断也非常安全
        if !meta.publish {
            continue;
        }
        
        println!(">>> 正在处理: {}", meta.title);
        let (content_html, plain_text) = get_page_html(&client, &page_id).await?;
        
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
            site_meta: SiteMeta {
                title: site_meta.title.clone(),
                icon_url: site_meta.icon_url.clone(),
                pages: site_meta.pages.clone(),
            },
            post: post_context,
            root_path: ".".to_string(),
        };
        
        let rendered = tera.render("post.html", &tera::Context::from_serialize(&context)?)?;
        fs::write(format!("public/{}", meta.url), rendered)?;
        
        if meta.in_list {
            posts_meta_for_index.push(meta);
        }
    }

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