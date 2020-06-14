// Copyright 2019 The xi-editor Authors.
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

//! A widget that provides simple visual styling options to a child.

use druid::widget::BackgroundBrush;
use druid::{
    BoxConstraints, Color, Data, Env, Event, EventCtx, KeyOrValue, LayoutCtx, LifeCycle,
    LifeCycleCtx, PaintCtx, Point, Rect, RenderContext, Size, UpdateCtx, Widget, WidgetPod,
};

struct BorderStyle {
    width: KeyOrValue<f64>,
    color: KeyOrValue<Color>,
}

/// A widget that provides simple visual styling options to a child.
pub struct Container<T> {
    background: Option<BackgroundBrush<T>>,
    border: Option<BorderStyle>,
    corner_radius: f64,
    inset: f64,

    inner: WidgetPod<T, Box<dyn Widget<T>>>,
}

pub trait ContainerParams {
    fn background<T>(&self) -> Option<BackgroundBrush<T>>;
    fn inset(&self) -> f64;
    fn corner_radius(&self) -> f64;
}

impl<T: Data + ContainerParams> Container<T> {
    /// Create Container with a child
    pub fn new(inner: impl Widget<T> + 'static) -> Self {
        Self {
            background: None,
            border: None,
            corner_radius: 0.0,
            inset: 0.0,
            inner: WidgetPod::new(inner).boxed(),
        }
    }
}

impl<T: Data + ContainerParams> Widget<T> for Container<T> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        self.inner.event(ctx, event, data, env);
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &T, env: &Env) {
        self.background = data.background();
        self.inset = data.inset();
        self.corner_radius = data.corner_radius();
        self.inner.lifecycle(ctx, event, data, env)
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &T, data: &T, env: &Env) {
        if !old_data.same(data) {
            self.background = data.background();
            self.inset = data.inset();
            self.corner_radius = data.corner_radius();
            ctx.request_layout();
        }
        self.inner.update(ctx, data, env);
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &T, env: &Env) -> Size {
        bc.debug_check("Container");

        // Shrink constraints by border offset
        let border_width = match &self.border {
            Some(border) => border.width.resolve(env),
            None => 0.0,
        };
        let padding = border_width + self.inset;

        let child_bc = bc.shrink((2. * padding, 2. * padding));
        let size = self.inner.layout(ctx, &child_bc, data, env);
        let origin = Point::new(padding, padding);
        self.inner
            .set_layout_rect(ctx, data, env, Rect::from_origin_size(origin, size));

        let my_size = Size::new(size.width + 2. * padding, size.height + 2. * padding);

        let my_insets = self.inner.compute_parent_paint_insets(my_size);
        ctx.set_paint_insets(my_insets);
        my_size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        if let Some(background) = self.background.as_mut() {
            let panel = ctx.size().to_rounded_rect(self.corner_radius);

            ctx.with_save(|ctx| {
                ctx.clip(panel);
                background.paint(ctx, data, env);
            });
        }

        if let Some(border) = &self.border {
            let border_width = border.width.resolve(env);
            let border_rect = ctx
                .size()
                .to_rect()
                .inset(border_width / -2.0)
                .to_rounded_rect(self.corner_radius);
            ctx.stroke(border_rect, &border.color.resolve(env), border_width);
        };

        self.inner.paint(ctx, data, env);
    }
}
