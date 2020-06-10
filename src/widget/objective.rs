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

use std::sync::Arc;

use druid::{
    piet::InterpolationMode, widget::FillStrat, BoxConstraints, Data, Env, Event, EventCtx,
    LayoutCtx, LifeCycle, LifeCycleCtx, PaintCtx, Rect, RenderContext, Size, UpdateCtx, Widget,
};

use crate::assets::{image::ImageData, IMAGES};
use crate::engine::{DisplayChild, ObjectiveState};

/// A widget that renders an Image
pub struct Objective {
    image: Option<Arc<ImageData>>,
}

impl Objective {
    /// Create an image drawing widget from `ImageData`.
    ///
    /// The Image will scale to fit its box constraints.
    pub fn new() -> Self {
        Objective { image: None }
    }

    fn update_image(&mut self, data: &DisplayChild) {
        let postfix = match data.state {
            ObjectiveState::Unlocked => "",
            ObjectiveState::Complete => ":completed",
            ObjectiveState::Locked => ":locked",
            _ => {
                self.image = None;
                return;
            }
        };

        let obj_id = format!("objective:{}{}", &data.id, &postfix);
        let ty_id = format!("type:{}{}", &data.ty, &postfix);

        IMAGES.with(|images| {
            self.image = images.borrow().get(&obj_id);
            // If there is no objective specific image, fall back on a type
            // specific one.
            if self.image.is_none() {
                self.image = images.borrow().get(&ty_id);
            }
        });
    }
}

impl Widget<DisplayChild> for Objective {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event, _data: &mut DisplayChild, _env: &Env) {
    }

    fn lifecycle(
        &mut self,
        _ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &DisplayChild,
        _env: &Env,
    ) {
        if let LifeCycle::WidgetAdded = event {
            self.update_image(data);
        }
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        old_data: &DisplayChild,
        data: &DisplayChild,
        _env: &Env,
    ) {
        if !old_data.same(data) {
            self.update_image(data);
            ctx.request_layout();
        }
    }

    fn layout(
        &mut self,
        _layout_ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _data: &DisplayChild,
        _env: &Env,
    ) -> Size {
        bc.debug_check("Image");

        if let Some(i) = &self.image {
            bc.constrain(i.get_size())
        } else {
            Size::ZERO
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, _data: &DisplayChild, _env: &Env) {
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
        }
    }
}
