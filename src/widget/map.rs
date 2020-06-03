// Copyright 2020 The xi-editor Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::cmp::min;
use std::sync::Arc;

use druid::{
    kurbo::Circle, piet::InterpolationMode, widget::FillStrat, BoxConstraints, Color, Data, Env,
    Event, EventCtx, LayoutCtx, LifeCycle, LifeCycleCtx, PaintCtx, Rect, RenderContext, Size,
    UpdateCtx, Widget,
};

use crate::assets::{image::ImageData, IMAGES};
use crate::engine::{DisplayChild, DisplayViewMap, MapInfo, ObjectiveState};

/// A widget that renders an Image
pub struct Map {
    image: Option<Arc<ImageData>>,
    scale: f64,
}

impl Map {
    /// Create an image drawing widget from `ImageData`.
    ///
    /// The Image will scale to fit its box constraints.
    pub fn new() -> Self {
        Map {
            image: None,
            scale: 1.0,
        }
    }

    fn update_map_image(&mut self, data: &MapInfo) {
        IMAGES.with(|images| {
            let id = format!("map:{}", &data.id);
            self.image = images.borrow().get(&id);
        });
    }
}

impl Widget<MapInfo> for Map {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event, _data: &mut MapInfo, _env: &Env) {}

    fn lifecycle(
        &mut self,
        _ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &MapInfo,
        _env: &Env,
    ) {
        if let LifeCycle::WidgetAdded = event {
            self.update_map_image(data);
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &MapInfo, data: &MapInfo, _env: &Env) {
        if !old_data.id.same(&data.id) {
            self.update_map_image(data);
            ctx.request_layout();
        }
    }

    fn layout(
        &mut self,
        _layout_ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _data: &MapInfo,
        _env: &Env,
    ) -> Size {
        bc.debug_check("Image");

        if let Some(i) = &self.image {
            let mut img_size = i.get_size();
            let x_scale = bc.max().width / img_size.width;
            let y_scale = bc.max().height / img_size.height;
            let scale = x_scale.min(y_scale);
            img_size.width *= scale;
            img_size.height *= scale;
            self.scale = scale;

            bc.constrain(img_size)
        } else {
            Size::ZERO
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &MapInfo, _env: &Env) {
        if let Some(i) = &self.image {
            let fill = FillStrat::default();
            let offset_matrix = fill.affine_to_fill(ctx.size(), i.get_size());

            // The ImageData's to_piet function does not clip to the image's size
            // CairoRenderContext is very like druids but with some extra goodies like clip
            if fill != FillStrat::Contain {
                let clip_rect = Rect::ZERO.with_size(ctx.size());
                ctx.clip(clip_rect);
            }
            i.to_piet(offset_matrix, ctx, InterpolationMode::Bilinear);

            let bg_color = Color::rgb8(0x00, 0x00, 0x00);
            let outline_color = Color::rgb8(0xff, 0xff, 0xff);

            let inner_radius = data.objective_radius * 0.6 * self.scale;
            let bg_radius = data.objective_radius * self.scale;
            let outline_radius = data.objective_radius * 0.8 * self.scale;
            let outline_width = data.objective_radius * 0.2 * self.scale;

            for objective in &*data.objectives {
                let pos = (objective.x * self.scale, objective.y * self.scale);
                let inner_circle = Circle::new(pos, inner_radius);
                let bg_circle = Circle::new(pos, bg_radius);
                let outline_circle = Circle::new(pos, outline_radius);

                let inner_color = Color::rgb8(0x00, 0xff, 0x00);

                ctx.fill(bg_circle, &bg_color);
                ctx.fill(inner_circle, &inner_color);
                ctx.stroke(outline_circle, &outline_color, outline_width);
            }
        }
    }
}
