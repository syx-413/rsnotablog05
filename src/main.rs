mod renderer;

use anyhow::{Context, Result};
use notionrs::Client;
use notionrs_types::prelude::*;
use renderer::HtmlRenderer;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::collections::HashMap;

// -----------------------------------------------------------
// 0. 配置结构
// -----------------------------------------------------------
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    url: String,
    notion_token: String,
    theme: String,
    title: Option<String>,
    description: Option<String>,
}

impl Config {
    fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path).context("无法读取配置文件")?;
        let config: Config = serde_json::from_str(&content).context("解析配置文件失败")?;
        Ok(config)
    }

    /// 从 URL 中提取 Notion ID (32位十六进制字符串)
    fn get_notion_id(&self) -> Result<String> {
        let parts: Vec<&str> = self.url.split('/').collect();
        let last_part = parts.last().ok_or_else(|| anyhow::anyhow!("无效的 URL"))?;
        
        // 处理带查询参数的 URL (例如 ?v=...)
        let id_part = last_part.split('?').next().unwrap();
        
        // Notion ID 应该是 32 位字符
        if id_part.len() >= 32 {
            Ok(id_part.to_string())
        } else {
            Err(anyhow::anyhow!("无法从 URL 提取有效的 Notion ID"))
        }
    }
}

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
// 0.5 渲染上下文
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

// -----------------------------------------------------------
// 1. 数据结构 (Notion API 响应映射)
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

    for block_res in response.results {
        let block_html = HtmlRenderer::render_block(&block_res.block);
        
        // 特殊处理 Toggle：我们需要把子内容放进 details 标签内部
        if let Block::Toggle { .. } = &block_res.block {
             // 移除末尾的 </details>
             let open_tag = block_html.strip_suffix("</details>").unwrap_or(&block_html);
             html.push_str(open_tag);
             
             if block_res.has_children {
                 let (children_html, children_text) = Box::pin(get_page_html(client, &block_res.id)).await?;
                 html.push_str("<div class=\"details-content\" style=\"padding-left: 1.2em;\">");
                 html.push_str(&children_html);
                 html.push_str("</div>");
                 if plain_text.len() < 200 {
                    plain_text.push_str(&children_text);
                 }
             }
             html.push_str("</details>");
        } else {
            // 普通 Block
            html.push_str(&block_html);
            html.push('\n');
            
            // 提取纯文本用于预览
            if plain_text.len() < 200 {
                plain_text.push_str(&block_res.block.to_string());
                plain_text.push(' ');
            }
            
            if block_res.has_children {
                let (children_html, children_text) = Box::pin(get_page_html(client, &block_res.id)).await?;
                html.push_str("<div style=\"margin-left: 20px;\">");
                html.push_str(&children_html);
                html.push_str("</div>");
                if plain_text.len() < 200 {
                    plain_text.push_str(&children_text);
                }
            }
        }
    }
    Ok((html, plain_text))
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 加载配置
    let config_path = "../notablog05/test-blog/config.json";
    let config = Config::load(config_path)?;
    let client = Client::new(&config.notion_token);
    let data_source_id = config.get_notion_id()?;

    // 2. 初始化 Tera 模板引擎
    let mut tera = tera::Tera::new("templates/**/*")?;
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
        let title = p.title.to_string();
        let safe_title = title.replace(" ", "_").replace("/", "-")
            .replace("?", "").replace(":", "").replace("*", "").replace("\"", "")
            .replace("<", "").replace(">", "").replace("|", "");
        let filename = format!("{}.html", safe_title);
        
        let date_str = p.date.date.as_ref()
            .and_then(|d| d.start.as_ref())
            .map(|dt| dt.to_string())
            .unwrap_or_else(|| "".to_string());

        // 提取页面图标 (Emoji 或 URL)
        let icon_url = match &page.icon {
            Some(Icon::Emoji(emoji)) => Some(emoji.emoji.clone()),
            Some(Icon::File(file_enum)) => match file_enum {
                // 尝试解构 external 字段
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
            tags: p.tags.multi_select.iter().map(|opt| Tag { 
                name: opt.name.clone(), 
                color: format!("{:?}", opt.color).to_lowercase(),
                slug: slugify(&opt.name)
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
        title: config.title.clone().unwrap_or_else(|| "My Blog".to_string()),
        icon_url: None,
        pages: all_posts.iter().map(|(_, m)| m.clone()).collect(),
    };

    fs::create_dir_all("public")?;

    // 4. 遍历处理每篇文章
    let mut posts_meta_for_index = Vec::new();
    for (page_id, mut meta) in all_posts {
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
    index_context.insert("pages", &posts_meta_for_index); // Changed from "posts" to "pages" to match articleList.html
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
        // 找到对应的标签颜色
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
    // 按数量降序排序
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
        context.insert("tagName", &tag_name); // 传入 tagName 供模板使用
        context.insert("pages", &tag_posts);
        context.insert("allTags", &all_tags); // 传入所有标签列表
        context.insert("rootPath", "..");
        
        // 优先使用 tag.html，如果没有则回退到 index.html
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
    
    // 自动拷贝 templates/assets 到 public/assets
    let assets_src = Path::new("templates/assets");
    if assets_src.exists() {
        println!(">>> 正在拷贝静态资源...");
        let assets_dst = Path::new("public/assets");
        copy_dir_recursive(assets_src, assets_dst)?;
    }

    println!(">>> 全部完成！请查看 public/index.html");

    Ok(())
}
