mod penevents;

use std::ops::Range;

use once_cell::sync::Lazy;
use p2d::bounding_volume::{BoundingVolume, AABB};
use piet::RenderContext;
use rnote_compose::helpers::{AABBHelpers, Vector2Helpers};
use rnote_compose::penhelpers::{KeyboardKey, PenEvent, PenState};
use rnote_compose::shapes::ShapeBehaviour;
use rnote_compose::style::drawhelpers;
use rnote_compose::{color, Transform};
use serde::{Deserialize, Serialize};

use crate::engine::{EngineView, EngineViewMut};
use crate::store::StrokeKey;
use crate::strokes::textstroke::{RangedTextAttribute, TextAttribute, TextStyle};
use crate::strokes::{Stroke, TextStroke};
use crate::{AudioPlayer, Camera, DrawOnDocBehaviour, WidgetFlags};

use super::penbehaviour::PenProgress;
use super::PenBehaviour;

#[derive(Debug, Clone)]
pub enum TypewriterState {
    Idle,
    Start(na::Vector2<f64>),
    Modifying {
        stroke_key: StrokeKey,
        cursor: unicode_segmentation::GraphemeCursor,
        pen_down: bool,
    },
    Selecting {
        stroke_key: StrokeKey,
        cursor: unicode_segmentation::GraphemeCursor,
        selection_cursor: unicode_segmentation::GraphemeCursor,
        /// If selecting is finished ( if true, will get reset on the next click )
        finished: bool,
    },
    Translating {
        stroke_key: StrokeKey,
        cursor: unicode_segmentation::GraphemeCursor,
        start_pos: na::Vector2<f64>,
        current_pos: na::Vector2<f64>,
    },
    AdjustTextWidth {
        stroke_key: StrokeKey,
        cursor: unicode_segmentation::GraphemeCursor,
        start_text_width: f64,
        start_pos: na::Vector2<f64>,
        current_pos: na::Vector2<f64>,
    },
}

impl Default for TypewriterState {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename = "typewriter")]
pub struct Typewriter {
    #[serde(rename = "text_style")]
    pub text_style: TextStyle,
    #[serde(rename = "max_width_enabled")]
    pub max_width_enabled: bool,
    #[serde(rename = "text_width")]
    pub text_width: f64,

    #[serde(skip)]
    state: TypewriterState,
}

impl Default for Typewriter {
    fn default() -> Self {
        Self {
            text_style: TextStyle::default(),
            max_width_enabled: true,
            text_width: 600.0,

            state: TypewriterState::default(),
        }
    }
}

impl DrawOnDocBehaviour for Typewriter {
    fn bounds_on_doc(&self, engine_view: &EngineView) -> Option<AABB> {
        let total_zoom = engine_view.camera.total_zoom();

        match &self.state {
            TypewriterState::Idle => None,
            TypewriterState::Start(pos) => Some(AABB::new(
                na::Point2::from(*pos),
                na::Point2::from(pos + na::vector![self.text_width, self.text_style.font_size]),
            )),
            TypewriterState::Modifying { stroke_key, .. }
            | TypewriterState::Selecting { stroke_key, .. }
            | TypewriterState::Translating { stroke_key, .. }
            | TypewriterState::AdjustTextWidth { stroke_key, .. } => {
                if let Some(Stroke::TextStroke(textstroke)) =
                    engine_view.store.get_stroke_ref(*stroke_key)
                {
                    let text_rect = Self::text_rect_bounds(self.text_width, textstroke);

                    let typewriter_bounds = text_rect.extend_by(
                        Self::TRANSLATE_NODE_SIZE.maxs(&Self::ADJUST_TEXT_WIDTH_NODE_SIZE)
                            / total_zoom,
                    );

                    Some(typewriter_bounds)
                } else {
                    None
                }
            }
        }
    }

    fn draw_on_doc(
        &self,
        cx: &mut piet_cairo::CairoRenderContext,
        engine_view: &EngineView,
    ) -> anyhow::Result<()> {
        cx.save().map_err(|e| anyhow::anyhow!("{}", e))?;

        static OUTLINE_COLOR: Lazy<piet::Color> =
            Lazy::new(|| color::GNOME_BRIGHTS[4].with_alpha(0.941));

        let total_zoom = engine_view.camera.total_zoom();

        let outline_width = 1.5 / total_zoom;
        let outline_corner_radius = 3.0 / total_zoom;

        match &self.state {
            TypewriterState::Idle => {}
            TypewriterState::Start(pos) => {
                if let Some(bounds) = self.bounds_on_doc(engine_view) {
                    let rect = bounds
                        .tightened(outline_width * 0.5)
                        .to_kurbo_rect()
                        .to_rounded_rect(outline_corner_radius);

                    cx.stroke(rect, &*OUTLINE_COLOR, outline_width);

                    let text = String::from("|");
                    let text_len = text.len();

                    // Draw the cursor
                    self.text_style.draw_cursor(
                        cx,
                        text,
                        &unicode_segmentation::GraphemeCursor::new(0, text_len, true),
                        &Transform::new_w_isometry(na::Isometry2::new(*pos, 0.0)),
                        engine_view.camera,
                    )?;
                }
            }
            TypewriterState::Modifying {
                stroke_key, cursor, ..
            } => {
                if let Some(Stroke::TextStroke(textstroke)) =
                    engine_view.store.get_stroke_ref(*stroke_key)
                {
                    let text_rect = Self::text_rect_bounds(self.text_width, textstroke);

                    let text_drawrect = text_rect
                        .tightened(outline_width * 0.5)
                        .to_kurbo_rect()
                        .to_rounded_rect(outline_corner_radius);

                    cx.stroke(text_drawrect, &*OUTLINE_COLOR, outline_width);

                    // Draw the cursor
                    textstroke.text_style.draw_cursor(
                        cx,
                        textstroke.text.clone(),
                        cursor,
                        &textstroke.transform,
                        engine_view.camera,
                    )?;

                    // Draw the text width adjust node
                    drawhelpers::draw_triangular_down_node(
                        cx,
                        PenState::Up,
                        Self::adjust_text_width_node_center(
                            text_rect.mins.coords,
                            self.text_width,
                            engine_view.camera,
                        ),
                        Self::ADJUST_TEXT_WIDTH_NODE_SIZE / total_zoom,
                        total_zoom,
                    );

                    if let Some(typewriter_bounds) = self.bounds_on_doc(engine_view) {
                        // draw translate Node
                        drawhelpers::draw_rectangular_node(
                            cx,
                            PenState::Up,
                            Self::translate_node_bounds(typewriter_bounds, engine_view.camera),
                            total_zoom,
                        );
                    }
                }
            }
            TypewriterState::Selecting {
                stroke_key,
                cursor,
                selection_cursor,
                ..
            } => {
                if let Some(Stroke::TextStroke(textstroke)) =
                    engine_view.store.get_stroke_ref(*stroke_key)
                {
                    let text_rect = Self::text_rect_bounds(self.text_width, textstroke);

                    let text_drawrect = text_rect
                        .tightened(outline_width * 0.5)
                        .to_kurbo_rect()
                        .to_rounded_rect(outline_corner_radius);

                    cx.stroke(text_drawrect, &*OUTLINE_COLOR, outline_width);

                    // Draw the text selection
                    textstroke.text_style.draw_text_selection(
                        cx,
                        textstroke.text.clone(),
                        cursor,
                        selection_cursor,
                        &textstroke.transform,
                        engine_view.camera,
                    );

                    // Draw the cursor
                    textstroke.text_style.draw_cursor(
                        cx,
                        textstroke.text.clone(),
                        cursor,
                        &textstroke.transform,
                        engine_view.camera,
                    )?;

                    // Draw the text width adjust node
                    drawhelpers::draw_triangular_down_node(
                        cx,
                        PenState::Up,
                        Self::adjust_text_width_node_center(
                            text_rect.mins.coords,
                            self.text_width,
                            engine_view.camera,
                        ),
                        Self::ADJUST_TEXT_WIDTH_NODE_SIZE / total_zoom,
                        total_zoom,
                    );

                    if let Some(typewriter_bounds) = self.bounds_on_doc(engine_view) {
                        // draw translate Node
                        drawhelpers::draw_rectangular_node(
                            cx,
                            PenState::Up,
                            Self::translate_node_bounds(typewriter_bounds, engine_view.camera),
                            total_zoom,
                        );
                    }
                }
            }
            TypewriterState::Translating { stroke_key, .. } => {
                if let Some(Stroke::TextStroke(textstroke)) =
                    engine_view.store.get_stroke_ref(*stroke_key)
                {
                    let text_rect = Self::text_rect_bounds(self.text_width, textstroke);

                    let text_drawrect = text_rect
                        .tightened(outline_width * 0.5)
                        .to_kurbo_rect()
                        .to_rounded_rect(outline_corner_radius);

                    cx.stroke(text_drawrect, &*OUTLINE_COLOR, outline_width);

                    // Draw the text width adjust node
                    drawhelpers::draw_triangular_down_node(
                        cx,
                        PenState::Up,
                        Self::adjust_text_width_node_center(
                            text_rect.mins.coords,
                            self.text_width,
                            engine_view.camera,
                        ),
                        Self::ADJUST_TEXT_WIDTH_NODE_SIZE / total_zoom,
                        total_zoom,
                    );

                    // Translate Node
                    if let Some(typewriter_bounds) = self.bounds_on_doc(engine_view) {
                        drawhelpers::draw_rectangular_node(
                            cx,
                            PenState::Down,
                            Self::translate_node_bounds(typewriter_bounds, engine_view.camera),
                            total_zoom,
                        );
                    }
                }
            }
            TypewriterState::AdjustTextWidth { stroke_key, .. } => {
                if let Some(Stroke::TextStroke(textstroke)) =
                    engine_view.store.get_stroke_ref(*stroke_key)
                {
                    let text_rect = Self::text_rect_bounds(self.text_width, textstroke);

                    let text_drawrect = text_rect
                        .tightened(outline_width * 0.5)
                        .to_kurbo_rect()
                        .to_rounded_rect(outline_corner_radius);

                    cx.stroke(text_drawrect, &*OUTLINE_COLOR, outline_width);

                    // Draw the text width adjust node
                    drawhelpers::draw_triangular_down_node(
                        cx,
                        PenState::Down,
                        Self::adjust_text_width_node_center(
                            text_rect.mins.coords,
                            self.text_width,
                            engine_view.camera,
                        ),
                        Self::ADJUST_TEXT_WIDTH_NODE_SIZE / total_zoom,
                        total_zoom,
                    );

                    // Translate Node
                    if let Some(typewriter_bounds) = self.bounds_on_doc(engine_view) {
                        drawhelpers::draw_rectangular_node(
                            cx,
                            PenState::Up,
                            Self::translate_node_bounds(typewriter_bounds, engine_view.camera),
                            total_zoom,
                        );
                    }
                }
            }
        }

        cx.restore().map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(())
    }
}

impl PenBehaviour for Typewriter {
    fn handle_event(
        &mut self,
        event: PenEvent,
        engine_view: &mut EngineViewMut,
    ) -> (PenProgress, WidgetFlags) {
        /*
               log::debug!(
                   "typewriter handle_event: state: {:#?}, event: {:#?}",
                   self.state,
                   event
               );
        */

        let (pen_progress, widget_flags) = match event {
            PenEvent::Down {
                element,
                shortcut_keys,
            } => self.handle_pen_event_down(element, shortcut_keys, engine_view),
            PenEvent::Up {
                element,
                shortcut_keys,
            } => self.handle_pen_event_up(element, shortcut_keys, engine_view),
            PenEvent::Proximity {
                element,
                shortcut_keys,
            } => self.handle_pen_event_proximity(element, shortcut_keys, engine_view),
            PenEvent::KeyPressed {
                keyboard_key,
                shortcut_keys,
            } => self.handle_pen_event_keypressed(keyboard_key, shortcut_keys, engine_view),
            PenEvent::Text { text } => self.handle_pen_event_text(text, engine_view),
            PenEvent::Cancel => self.handle_pen_event_cancel(engine_view),
        };

        (pen_progress, widget_flags)
    }

    fn fetch_clipboard_content(
        &self,
        engine_view: &EngineView,
    ) -> anyhow::Result<Option<(Vec<u8>, String)>> {
        match &self.state {
            TypewriterState::Idle
            | TypewriterState::Start(_)
            | TypewriterState::Modifying { .. }
            | TypewriterState::Translating { .. }
            | TypewriterState::AdjustTextWidth { .. } => Ok(None),
            TypewriterState::Selecting {
                stroke_key,
                cursor,
                selection_cursor,
                ..
            } => {
                if let Some(Stroke::TextStroke(textstroke)) =
                    engine_view.store.get_stroke_ref(*stroke_key)
                {
                    let cursor_pos = cursor.cur_cursor();
                    let selection_cursor_pos = selection_cursor.cur_cursor();

                    // ensure range is positive
                    let pos_cursor_range = if cursor_pos < selection_cursor_pos {
                        cursor_pos..selection_cursor_pos
                    } else {
                        selection_cursor_pos..cursor_pos
                    };

                    // Current selection as clipboard text
                    let selection_text = textstroke
                        .get_text_slice_for_range(pos_cursor_range)
                        .to_string();

                    return Ok(Some((
                        selection_text.into_bytes(),
                        String::from("text/plain;charset=utf-8"),
                    )));
                }

                Ok(None)
            }
        }
    }

    fn update_internal_state(&mut self, engine_view: &EngineView) {
        match &mut self.state {
            TypewriterState::Idle | TypewriterState::Start(_) => {}
            TypewriterState::Selecting {
                stroke_key,
                cursor,
                selection_cursor,
                ..
            } => {
                if let Some(Stroke::TextStroke(textstroke)) =
                    engine_view.store.get_stroke_ref(*stroke_key)
                {
                    self.text_style = textstroke.text_style.clone();
                    self.text_style.ranged_text_attributes.clear();

                    if let Some(max_width) = textstroke.text_style.max_width {
                        self.text_width = max_width;
                    }

                    update_cursors_for_textstroke(textstroke, cursor, Some(selection_cursor));
                }
            }
            TypewriterState::Modifying {
                stroke_key, cursor, ..
            }
            | TypewriterState::Translating {
                stroke_key, cursor, ..
            }
            | TypewriterState::AdjustTextWidth {
                stroke_key, cursor, ..
            } => {
                if let Some(Stroke::TextStroke(textstroke)) =
                    engine_view.store.get_stroke_ref(*stroke_key)
                {
                    self.text_style = textstroke.text_style.clone();
                    self.text_style.ranged_text_attributes.clear();

                    if let Some(max_width) = textstroke.text_style.max_width {
                        self.text_width = max_width;
                    }

                    update_cursors_for_textstroke(textstroke, cursor, None);
                }
            }
        }
    }
}

// Updates the cursors to valid positions and new text length.
fn update_cursors_for_textstroke(
    textstroke: &TextStroke,
    cursor: &mut unicode_segmentation::GraphemeCursor,
    selection_cursor: Option<&mut unicode_segmentation::GraphemeCursor>,
) {
    *cursor = unicode_segmentation::GraphemeCursor::new(
        cursor.cur_cursor().min(textstroke.text.len()),
        textstroke.text.len(),
        true,
    );
    if let Some(selection_cursor) = selection_cursor {
        *selection_cursor = unicode_segmentation::GraphemeCursor::new(
            selection_cursor.cur_cursor().min(textstroke.text.len()),
            textstroke.text.len(),
            true,
        );
    }
}

impl Typewriter {
    // The size of the translate node, located in the upper left corner
    const TRANSLATE_NODE_SIZE: na::Vector2<f64> = na::vector![18.0, 18.0];
    /// The threshold where a translation is applied ( in offset magnitude, surface coords )
    const TRANSLATE_MAGNITUDE_THRESHOLD: f64 = 1.0;
    // The size of the translate node, located in the upper left corner
    const ADJUST_TEXT_WIDTH_NODE_SIZE: na::Vector2<f64> = na::vector![18.0, 18.0];

    fn start_audio(keyboard_key: Option<KeyboardKey>, audioplayer: &mut Option<AudioPlayer>) {
        if let Some(audioplayer) = audioplayer {
            audioplayer.play_typewriter_key_sound(keyboard_key);
        }
    }

    /// the bounds of the text rect enclosing the textstroke
    fn text_rect_bounds(text_width: f64, textstroke: &TextStroke) -> AABB {
        let origin = textstroke.transform.translation_part();

        AABB::new(
            na::Point2::from(origin),
            na::point![origin[0] + text_width, origin[1]],
        )
        .merged(&textstroke.bounds())
    }

    /// the bounds of the translate node
    fn translate_node_bounds(typewriter_bounds: AABB, camera: &Camera) -> AABB {
        let total_zoom = camera.total_zoom();

        AABB::from_half_extents(
            na::Point2::from(
                typewriter_bounds.mins.coords + Self::TRANSLATE_NODE_SIZE * 0.5 / total_zoom,
            ),
            Self::TRANSLATE_NODE_SIZE * 0.5 / total_zoom,
        )
    }

    /// the center of the adjust text width node
    fn adjust_text_width_node_center(
        text_rect_origin: na::Vector2<f64>,
        text_width: f64,
        camera: &Camera,
    ) -> na::Vector2<f64> {
        let total_zoom = camera.total_zoom();

        na::vector![
            text_rect_origin[0] + text_width,
            text_rect_origin[1] - Self::ADJUST_TEXT_WIDTH_NODE_SIZE[1] * 0.5 / total_zoom
        ]
    }

    /// the bounds of the adjust text width node
    fn adjust_text_width_node_bounds(
        text_rect_origin: na::Vector2<f64>,
        text_width: f64,
        camera: &Camera,
    ) -> AABB {
        let total_zoom = camera.total_zoom();
        let center = Self::adjust_text_width_node_center(text_rect_origin, text_width, camera);

        AABB::from_half_extents(
            na::Point2::from(center),
            Self::ADJUST_TEXT_WIDTH_NODE_SIZE * 0.5 / total_zoom,
        )
    }

    /// Returns the range of the current selection, if available
    pub fn selection_range(&self) -> Option<(Range<usize>, StrokeKey)> {
        if let TypewriterState::Selecting {
            stroke_key,
            cursor,
            selection_cursor,
            ..
        } = &self.state
        {
            let cursor_index = cursor.cur_cursor();
            let selection_cursor_index = selection_cursor.cur_cursor();

            if cursor_index < selection_cursor_index {
                Some((cursor_index..selection_cursor_index, *stroke_key))
            } else {
                Some((selection_cursor_index..cursor_index, *stroke_key))
            }
        } else {
            None
        }
    }

    /// Inserts text either at the current cursor position or,
    /// if the state is idle, a new textstroke (at the preferred position, if supplied. Else at a default offset).
    pub fn insert_text(
        &mut self,
        text: String,
        preferred_pos: Option<na::Vector2<f64>>,
        engine_view: &mut EngineViewMut,
    ) -> WidgetFlags {
        let pos = preferred_pos.unwrap_or_else(|| {
            engine_view.camera.viewport().mins.coords + Stroke::IMPORT_OFFSET_DEFAULT
        });
        let mut widget_flags = WidgetFlags::default();

        match &mut self.state {
            TypewriterState::Idle => {
                let text_len = text.len();

                widget_flags.merge_with_other(engine_view.store.record());

                let mut text_style = self.text_style.clone();
                text_style.ranged_text_attributes.clear();

                if self.max_width_enabled {
                    text_style.max_width = Some(self.text_width);
                }

                let textstroke = TextStroke::new(text, pos, text_style);

                let cursor = unicode_segmentation::GraphemeCursor::new(
                    text_len,
                    textstroke.text.len(),
                    true,
                );

                let stroke_key = engine_view
                    .store
                    .insert_stroke(Stroke::TextStroke(textstroke), None);

                if let Err(e) = engine_view.store.regenerate_rendering_for_stroke(
                    stroke_key,
                    engine_view.camera.viewport(),
                    engine_view.camera.image_scale(),
                ) {
                    log::error!("regenerate_rendering_for_stroke() after inserting a new textstroke from clipboard contents in typewriter paste_clipboard_contents() failed with Err {}", e);
                }

                self.state = TypewriterState::Modifying {
                    stroke_key,
                    cursor,
                    pen_down: false,
                };

                widget_flags.redraw = true;
            }
            TypewriterState::Start(pos) => {
                let text_len = text.len();

                widget_flags.merge_with_other(engine_view.store.record());

                let mut text_style = self.text_style.clone();
                text_style.ranged_text_attributes.clear();

                if self.max_width_enabled {
                    text_style.max_width = Some(self.text_width);
                }

                let textstroke = TextStroke::new(text, *pos, text_style);

                let cursor = unicode_segmentation::GraphemeCursor::new(
                    text_len,
                    textstroke.text.len(),
                    true,
                );

                let stroke_key = engine_view
                    .store
                    .insert_stroke(Stroke::TextStroke(textstroke), None);

                if let Err(e) = engine_view.store.regenerate_rendering_for_stroke(
                    stroke_key,
                    engine_view.camera.viewport(),
                    engine_view.camera.image_scale(),
                ) {
                    log::error!("regenerate_rendering_for_stroke() after inserting a new textstroke from clipboard contents in typewriter paste_clipboard_contents() failed with Err {}", e);
                }

                self.state = TypewriterState::Modifying {
                    stroke_key,
                    cursor,
                    pen_down: false,
                };

                widget_flags.redraw = true;
            }
            TypewriterState::Modifying {
                stroke_key, cursor, ..
            } => {
                widget_flags.merge_with_other(engine_view.store.record());

                if let Some(Stroke::TextStroke(textstroke)) =
                    engine_view.store.get_stroke_mut(*stroke_key)
                {
                    textstroke.insert_text_after_cursor(text.as_str(), cursor);

                    engine_view.store.update_geometry_for_stroke(*stroke_key);
                    engine_view.store.regenerate_rendering_for_stroke_threaded(
                        engine_view.tasks_tx.clone(),
                        *stroke_key,
                        engine_view.camera.viewport(),
                        engine_view.camera.image_scale(),
                    );

                    engine_view
                        .doc
                        .resize_autoexpand(engine_view.store, engine_view.camera);

                    widget_flags.redraw = true;
                    widget_flags.resize = true;
                    widget_flags.indicate_changed_store = true;
                }
            }
            TypewriterState::Selecting {
                stroke_key,
                cursor,
                selection_cursor,
                ..
            } => {
                widget_flags.merge_with_other(engine_view.store.record());

                if let Some(Stroke::TextStroke(textstroke)) =
                    engine_view.store.get_stroke_mut(*stroke_key)
                {
                    textstroke.replace_text_between_selection_cursors(
                        cursor,
                        selection_cursor,
                        text.as_str(),
                    );

                    engine_view.store.update_geometry_for_stroke(*stroke_key);
                    engine_view.store.regenerate_rendering_for_stroke_threaded(
                        engine_view.tasks_tx.clone(),
                        *stroke_key,
                        engine_view.camera.viewport(),
                        engine_view.camera.image_scale(),
                    );
                    engine_view
                        .doc
                        .resize_autoexpand(engine_view.store, engine_view.camera);

                    self.state = TypewriterState::Modifying {
                        stroke_key: *stroke_key,
                        cursor: cursor.clone(),
                        pen_down: false,
                    };

                    widget_flags.resize = true;
                    widget_flags.redraw = true;
                    widget_flags.indicate_changed_store = true;
                }
            }
            TypewriterState::Translating { .. } | TypewriterState::AdjustTextWidth { .. } => {}
        }

        widget_flags
    }

    // changes the text style of the text stroke that is currently being modified
    pub fn change_text_style_in_modifying_stroke<F>(
        &mut self,
        modify_func: F,
        engine_view: &mut EngineViewMut,
    ) -> WidgetFlags
    where
        F: FnOnce(&mut TextStyle),
    {
        let mut widget_flags = WidgetFlags::default();

        if let TypewriterState::Modifying { stroke_key, .. }
        | TypewriterState::Selecting { stroke_key, .. }
        | TypewriterState::Translating { stroke_key, .. }
        | TypewriterState::AdjustTextWidth { stroke_key, .. } = &mut self.state
        {
            widget_flags.merge_with_other(engine_view.store.record());

            if let Some(Stroke::TextStroke(textstroke)) =
                engine_view.store.get_stroke_mut(*stroke_key)
            {
                modify_func(&mut textstroke.text_style);

                engine_view.store.update_geometry_for_stroke(*stroke_key);
                if let Err(e) = engine_view.store.regenerate_rendering_for_stroke(
                    *stroke_key,
                    engine_view.camera.viewport(),
                    engine_view.camera.image_scale(),
                ) {
                    log::error!("regenerate_rendering_for_stroke() failed with Err {}", e);
                }

                widget_flags.redraw = true;
                widget_flags.indicate_changed_store = true;
            }
        }

        widget_flags
    }

    pub fn remove_text_attributes_current_selection(
        &mut self,
        engine_view: &mut EngineViewMut,
    ) -> WidgetFlags {
        let mut widget_flags = WidgetFlags::default();

        if let Some((selection_range, stroke_key)) = self.selection_range() {
            widget_flags.merge_with_other(engine_view.store.record());

            if let Some(Stroke::TextStroke(textstroke)) =
                engine_view.store.get_stroke_mut(stroke_key)
            {
                textstroke.remove_attrs_for_range(selection_range);

                engine_view.store.update_geometry_for_stroke(stroke_key);
                if let Err(e) = engine_view.store.regenerate_rendering_for_stroke(
                    stroke_key,
                    engine_view.camera.viewport(),
                    engine_view.camera.image_scale(),
                ) {
                    log::error!("regenerate_rendering_for_stroke() failed with Err {}", e);
                }

                widget_flags.redraw = true;
                widget_flags.indicate_changed_store = true;
            }
        }

        widget_flags
    }

    pub fn add_text_attribute_current_selection(
        &mut self,
        text_attribute: TextAttribute,
        engine_view: &mut EngineViewMut,
    ) -> WidgetFlags {
        let mut widget_flags = WidgetFlags::default();

        if let Some((selection_range, stroke_key)) = self.selection_range() {
            widget_flags.merge_with_other(engine_view.store.record());

            if let Some(Stroke::TextStroke(textstroke)) =
                engine_view.store.get_stroke_mut(stroke_key)
            {
                textstroke
                    .text_style
                    .ranged_text_attributes
                    .push(RangedTextAttribute {
                        attribute: text_attribute,
                        range: selection_range,
                    });

                engine_view.store.update_geometry_for_stroke(stroke_key);
                if let Err(e) = engine_view.store.regenerate_rendering_for_stroke(
                    stroke_key,
                    engine_view.camera.viewport(),
                    engine_view.camera.image_scale(),
                ) {
                    log::error!("regenerate_rendering_for_stroke() failed with Err {}", e);
                }

                widget_flags.redraw = true;
                widget_flags.indicate_changed_store = true;
            }
        }

        widget_flags
    }
}
