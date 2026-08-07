//! Design Mode overlays for the embedded browser (Cursor-aligned MVP).

use extension_cef::DesignNodeInfo;
use gpui::{Bounds, Pixels, Point, px};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default)]
pub struct DesignModeState {
    pub enabled: bool,
    pub selected: Vec<DesignSelection>,
    pub draft_region: Option<RegionAnnotation>,
    pub dragging_from: Option<Point<Pixels>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DesignSelection {
    pub node: DesignNodeInfo,
    pub screenshot_note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegionAnnotation {
    pub bounds: SerializedBounds,
    pub note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializedBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<Bounds<Pixels>> for SerializedBounds {
    fn from(bounds: Bounds<Pixels>) -> Self {
        Self {
            x: bounds.origin.x.into(),
            y: bounds.origin.y.into(),
            width: bounds.size.width.into(),
            height: bounds.size.height.into(),
        }
    }
}

impl DesignModeState {
    pub fn clear(&mut self) {
        self.selected.clear();
        self.draft_region = None;
        self.dragging_from = None;
    }

    pub fn select_node(&mut self, node: DesignNodeInfo, additive: bool) {
        let selection = DesignSelection {
            screenshot_note: format!("viewport point ({}, {})", node.x, node.y),
            node,
        };
        if additive {
            self.selected.push(selection);
        } else {
            self.selected = vec![selection];
        }
    }

    pub fn begin_region(&mut self, origin: Point<Pixels>) {
        self.dragging_from = Some(origin);
        self.draft_region = Some(RegionAnnotation {
            bounds: SerializedBounds {
                x: origin.x.into(),
                y: origin.y.into(),
                width: 0.,
                height: 0.,
            },
            note: String::new(),
        });
    }

    pub fn update_region(&mut self, current: Point<Pixels>) {
        let Some(origin) = self.dragging_from else {
            return;
        };
        let min_x = origin.x.min(current.x);
        let min_y = origin.y.min(current.y);
        let width = (origin.x - current.x).abs();
        let height = (origin.y - current.y).abs();
        if let Some(region) = &mut self.draft_region {
            region.bounds = SerializedBounds {
                x: min_x.into(),
                y: min_y.into(),
                width: width.into(),
                height: height.into(),
            };
        }
    }

    pub fn finish_region(&mut self) {
        self.dragging_from = None;
        if let Some(region) = &mut self.draft_region {
            if region.bounds.width < 4. || region.bounds.height < 4. {
                self.draft_region = None;
            } else {
                region.note = format!(
                    "annotated region {}x{} at ({}, {})",
                    region.bounds.width, region.bounds.height, region.bounds.x, region.bounds.y
                );
            }
        }
    }

    /// Markdown/text block suitable for agent context attachment.
    pub fn agent_context_markdown(&self) -> String {
        let mut out = String::from("## Design Mode context\n");
        if self.selected.is_empty() && self.draft_region.is_none() {
            out.push_str("(no elements selected)\n");
            return out;
        }
        for (index, selection) in self.selected.iter().enumerate() {
            out.push_str(&format!(
                "\n### Selection {}\n- tag: `{}`\n- xpath: `{}`\n- styles: {}\n- attrs: {:?}\n- note: {}\n",
                index + 1,
                selection.node.tag,
                selection.node.xpath,
                selection.node.computed_style_summary,
                selection.node.attributes,
                selection.screenshot_note
            ));
        }
        if let Some(region) = &self.draft_region {
            out.push_str(&format!(
                "\n### Region\n- bounds: ({}, {}) {}×{}\n- note: {}\n",
                region.bounds.x,
                region.bounds.y,
                region.bounds.width,
                region.bounds.height,
                region.note
            ));
        }
        out
    }

    pub fn region_bounds_px(&self) -> Option<Bounds<Pixels>> {
        let region = self.draft_region.as_ref()?;
        Some(Bounds {
            origin: Point {
                x: px(region.bounds.x),
                y: px(region.bounds.y),
            },
            size: gpui::Size {
                width: px(region.bounds.width),
                height: px(region.bounds.height),
            },
        })
    }
}
