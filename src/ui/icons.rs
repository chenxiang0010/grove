//! The icon set: JetBrains' own 16pt glyphs, drawn through `svg()`.
//!
//! The files under `assets/icons/` are the reference IDE's new-UI icons
//! (Apache-2.0), with their two greys rewritten to `currentColor` so a glyph
//! takes the text colour of whatever holds it — the same rule the reference
//! uses for its monochrome icons. A coloured icon (a star, a tag) keeps its
//! palette.
//!
//! An icon is one `svg()` element: no image loading, no per-size raster, and
//! the platform draws the shapes at the device's pixels.

use nsh_core::element::ImageSrc;
use nsh_core::prelude::*;

/// One glyph of the set.
///
/// The catalogue is the reference IDE's whole UI icon set, vendored, so it
/// holds glyphs no screen draws yet; the allow keeps the set whole rather
/// than deleting a glyph a later screen will want.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Project,
    Git,
    Commit,
    Changes,
    Structure,
    Terminal,
    Settings,
    Refresh,
    Fetch,
    Push,
    Pull,
    Branch,
    Tag,
    CurrentBranch,
    Star,
    StarOutline,
    Stash,
    Shelve,
    Folder,
    File,
    Add,
    Remove,
    Delete,
    Close,
    Search,
    Filter,
    History,
    Show,
    More,
    ChevronDown,
    ChevronRight,
    ChevronLeft,
    CollapseAll,
    ExpandAll,
    Locate,
    Copy,
    Bookmark,
    Sort,
    Diff,
    Revert,
    Check,
    Regex,
    Case,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Incoming,
    Outgoing,
    Undo,
    Redo,
    Module,
    Package,
    Menu,
    FolderOpen,
}

/// The main menu's three lines, which the set has no file for.
const MENU_SVG: &str = r##"<svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg"><rect x="2" y="4" width="12" height="1.5" rx="0.75" fill="currentColor"/><rect x="2" y="7.25" width="12" height="1.5" rx="0.75" fill="currentColor"/><rect x="2" y="10.5" width="12" height="1.5" rx="0.75" fill="currentColor"/></svg>"##;

impl Icon {
    /// The SVG this glyph is drawn from.
    pub fn markup(self) -> &'static str {
        match self {
            Self::Project => include_str!("../../assets/icons/project.svg"),
            Self::Git => include_str!("../../assets/icons/git.svg"),
            Self::Commit => include_str!("../../assets/icons/commit.svg"),
            Self::Changes => include_str!("../../assets/icons/changes.svg"),
            Self::Structure => include_str!("../../assets/icons/structure.svg"),
            Self::Terminal => include_str!("../../assets/icons/terminal.svg"),
            Self::Settings => include_str!("../../assets/icons/settings.svg"),
            Self::Refresh => include_str!("../../assets/icons/refresh.svg"),
            Self::Fetch => include_str!("../../assets/icons/fetch.svg"),
            Self::Push => include_str!("../../assets/icons/push.svg"),
            Self::Pull => include_str!("../../assets/icons/pull.svg"),
            Self::Branch => include_str!("../../assets/icons/branch.svg"),
            Self::Tag => include_str!("../../assets/icons/tag.svg"),
            Self::CurrentBranch => include_str!("../../assets/icons/current-branch.svg"),
            Self::Star => include_str!("../../assets/icons/star.svg"),
            Self::StarOutline => include_str!("../../assets/icons/star-outline.svg"),
            Self::Stash => include_str!("../../assets/icons/stash.svg"),
            Self::Shelve => include_str!("../../assets/icons/shelve.svg"),
            Self::Folder => include_str!("../../assets/icons/folder.svg"),
            Self::File => include_str!("../../assets/icons/file.svg"),
            Self::Add => include_str!("../../assets/icons/add.svg"),
            Self::Remove => include_str!("../../assets/icons/remove.svg"),
            Self::Delete => include_str!("../../assets/icons/delete.svg"),
            Self::Close => include_str!("../../assets/icons/close.svg"),
            Self::Search => include_str!("../../assets/icons/search.svg"),
            Self::Filter => include_str!("../../assets/icons/filter.svg"),
            Self::History => include_str!("../../assets/icons/history.svg"),
            Self::Show => include_str!("../../assets/icons/show.svg"),
            Self::More => include_str!("../../assets/icons/more.svg"),
            Self::ChevronDown => include_str!("../../assets/icons/chevron-down.svg"),
            Self::ChevronRight => include_str!("../../assets/icons/chevron-right.svg"),
            Self::ChevronLeft => include_str!("../../assets/icons/chevron-left.svg"),
            Self::CollapseAll => include_str!("../../assets/icons/collapse-all.svg"),
            Self::ExpandAll => include_str!("../../assets/icons/expand-all.svg"),
            Self::Locate => include_str!("../../assets/icons/locate.svg"),
            Self::Copy => include_str!("../../assets/icons/copy.svg"),
            Self::Bookmark => include_str!("../../assets/icons/bookmark.svg"),
            Self::Sort => include_str!("../../assets/icons/sort.svg"),
            Self::Diff => include_str!("../../assets/icons/diff.svg"),
            Self::Revert => include_str!("../../assets/icons/revert.svg"),
            Self::Check => include_str!("../../assets/icons/check.svg"),
            Self::Regex => include_str!("../../assets/icons/regex.svg"),
            Self::Case => include_str!("../../assets/icons/case.svg"),
            Self::ArrowLeft => include_str!("../../assets/icons/arrow-left.svg"),
            Self::ArrowRight => include_str!("../../assets/icons/arrow-right.svg"),
            Self::ArrowUp => include_str!("../../assets/icons/arrow-up.svg"),
            Self::ArrowDown => include_str!("../../assets/icons/arrow-down.svg"),
            Self::Incoming => include_str!("../../assets/icons/incoming.svg"),
            Self::Outgoing => include_str!("../../assets/icons/outgoing.svg"),
            Self::Undo => include_str!("../../assets/icons/undo.svg"),
            Self::Redo => include_str!("../../assets/icons/redo.svg"),
            Self::Module => include_str!("../../assets/icons/module.svg"),
            Self::Package => include_str!("../../assets/icons/package.svg"),
            Self::Menu => MENU_SVG,
            Self::FolderOpen => include_str!("../../assets/icons/folder.svg"),
        }
    }
}

/// The file-type icon for `name`: the reference IDE's own per-extension
/// pictures, so a tree reads at a glance.
pub fn file_icon_markup(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or(name).to_ascii_lowercase();
    match ext.as_str() {
        "rs" => include_str!("../../assets/icons/files/rust.svg"),
        "md" | "markdown" => include_str!("../../assets/icons/files/markdown.svg"),
        "json" => include_str!("../../assets/icons/files/json.svg"),
        "yaml" | "yml" => include_str!("../../assets/icons/files/yaml.svg"),
        "toml" => include_str!("../../assets/icons/files/toml.svg"),
        "py" => include_str!("../../assets/icons/files/python.svg"),
        "js" | "mjs" | "cjs" | "jsx" => include_str!("../../assets/icons/files/javascript.svg"),
        "ts" | "tsx" | "mts" => include_str!("../../assets/icons/files/typescript.svg"),
        "html" | "htm" => include_str!("../../assets/icons/files/html.svg"),
        "css" | "scss" | "less" => include_str!("../../assets/icons/files/css.svg"),
        "sql" => include_str!("../../assets/icons/files/sql.svg"),
        "sh" | "bash" | "zsh" | "fish" => include_str!("../../assets/icons/files/shell.svg"),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico" => {
            include_str!("../../assets/icons/files/image.svg")
        }
        "xml" | "xsd" => include_str!("../../assets/icons/files/xml.svg"),
        "java" => include_str!("../../assets/icons/files/java.svg"),
        "kt" | "kts" => include_str!("../../assets/icons/files/kotlin.svg"),
        "gradle" => include_str!("../../assets/icons/files/gradle.svg"),
        "properties" | "ini" | "conf" => include_str!("../../assets/icons/files/properties.svg"),
        "zip" | "gz" | "tar" | "xz" | "zst" | "7z" => {
            include_str!("../../assets/icons/files/archive.svg")
        }
        "csv" => include_str!("../../assets/icons/files/csv.svg"),
        "ttf" | "otf" | "woff" | "woff2" => include_str!("../../assets/icons/files/font.svg"),
        "lock" => include_str!("../../assets/icons/files/lock.svg"),
        "patch" | "diff" => include_str!("../../assets/icons/files/patch.svg"),
        "dockerfile" => include_str!("../../assets/icons/files/docker.svg"),
        _ => match name {
            ".gitignore" => include_str!("../../assets/icons/files/gitignore.svg"),
            "Cargo.toml" | "Cargo.lock" => include_str!("../../assets/icons/files/cargo.svg"),
            _ => include_str!("../../assets/icons/files/text.svg"),
        },
    }
}

/// A file's icon, by name.
pub fn file_icon(name: &str, size: f32) -> Svg {
    svg(ImageSrc::svg_markup(file_icon_markup(name)))
        .size(px(size))
        .flex_none()
}

/// A glyph at `size` points, in the text colour it inherits.
pub fn icon(glyph: Icon, size: f32) -> Svg {
    svg(ImageSrc::svg_markup(glyph.markup()))
        .size(px(size))
        .flex_none()
}

/// A glyph that takes whatever text colour its container sets.
#[allow(dead_code)]
pub fn tinted_icon(glyph: Icon, size: f32, color: impl Fn() -> AdaptiveColor + 'static) -> Svg {
    icon(glyph, size).text_color(color)
}
