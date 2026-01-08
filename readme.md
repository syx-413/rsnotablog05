# rsnotablog

[![Rust](https://img.shields.io/badge/Made%20with-Rust-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)

**rsnotablog** 是一个基于 Rust 编写的高性能静态博客生成器，专为 Notion 用户打造。它能将你的 Notion 页面无缝转换为极简、美观且响应式的静态网站。

它是原版 Node.js [notablog](https://github.com/dragonman225/notablog) 的 Rust 重构与增强版，旨在提供极致的构建速度和零依赖的部署体验。

## ✨ 核心特性

- **🚀 极速构建**：利用 Rust 的异步并发能力，秒级生成大量静态页面。
- **📝 Notion 驱动**：直接使用 Notion 作为 CMS，享受“所见即所得”的写作体验。
- **🎨 完美复刻**：内置经典的 `pure-ejs` 主题，保留优雅的排版和交互。
- **🧩 全面支持**：
    - **排版**：标题、列表、引用、分割线、加粗/斜体/下划线。
    - **交互**：Callout 提示框、Toggle 折叠列表（支持嵌套）。
    - **媒体**：图片、视频 (Video)、音频 (Audio)、PDF 预览、文件下载。
    - **嵌入**：支持 Bookmark 书签卡片、通用 Embed（如 YouTube/Bilibili iframe）。
    - **学术**：集成 **KaTeX**，完美渲染块级和行内数学公式。
    - **代码**：集成 **Prism.js**，支持多种编程语言的高亮显示。
- **🏷️ 标签系统**：自动提取文章标签，生成独立的标签分类页面。
- **💬 评论系统**：内置 Utterances 评论支持。
- **📱 响应式设计**：完美适配移动端和桌面端阅读。

## 🚀 快速开始

### 1. 准备工作

确保你已经安装了 [Rust](https://www.rust-lang.org/tools/install) (Cargo)。

### 2. 配置 Notion

1.  Duplicate [这个 Notion 模板](https://www.notion.so/b6fcf809ca5047b89f423948dce013a0) 到你的工作区。
2.  创建一个 Notion Integration，并获取 `Internal Integration Token`。
3.  在 Notion 中将该数据库 Share 给你的 Integration。
4.  修改 `notablog05/test-blog/config.json` (或在代码中指定路径)：

```json
{
  "url": "https://www.notion.so/your-database-id",
  "notionToken": "secret_your_notion_token",
  "theme": "pure-ejs"
}
```

### 3. 运行生成

在项目根目录下运行：

```bash
cd rsnotablog05
cargo run
```

### 4. 预览与部署

构建完成后，静态网站生成在 `rsnotablog05/public` 目录。

- **本地预览**：直接用浏览器打开 `rsnotablog05/public/index.html`。
- **部署**：将 `public` 文件夹内容推送到 GitHub Pages、Vercel 或 Netlify。

## 📂 项目结构

```
rsnotablog05/
├── src/
│   ├── main.rs        # 核心逻辑：配置读取、Notion API 抓取、页面生成
│   └── renderer.rs    # 渲染器：将 Notion Block 转换为 HTML
├── templates/         # Tera 模板文件
│   ├── index.html     # 首页
│   ├── post.html      # 文章页
│   ├── partials/      # 组件 (Header, Navbar, Footer, ArticleList)
│   └── assets/        # 静态资源 (CSS, JS, Fonts)
├── public/            # [生成目录] 最终的静态网站
└── Cargo.toml         # 依赖配置
```

## 🛠️ 自定义样式

所有样式文件位于 `templates/assets/css/`。
- **CustomSetting.css**：推荐在此文件中进行自定义修改，它会覆盖默认样式。
- **notablog.css**：核心布局样式。
- **theme.css**：颜色与字体主题。

## 📝 待办事项

- [x] 完整 Block 类型支持 (Image, Video, Audio, Pdf, Bookmark, Toggle)
- [x] 标签分类页生成
- [x] 静态资源自动拷贝
- [x] 页面图标提取 (Emoji/Image)
- [ ] 增量构建 (缓存机制)
- [ ] RSS / Atom Feed 生成
- [ ] SEO 优化 (Sitemap, Meta tags)

## 📄 许可证

MIT License
