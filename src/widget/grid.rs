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

use std::cmp::Ordering;
use std::sync::Arc;

use druid::kurbo::{Point, Rect, Size};
use druid::{
    BoxConstraints, Data, Env, Event, EventCtx, LayoutCtx, LifeCycle, LifeCycleCtx, PaintCtx,
    UpdateCtx, Widget, WidgetPod,
};

use crate::engine::{DisplayChild, DisplayViewGrid};

/// A list widget for a variable-size collection of items.
pub struct Grid {
    closure: Box<dyn Fn() -> Box<dyn Widget<DisplayChild>>>,
    children: Vec<WidgetPod<DisplayChild, Box<dyn Widget<DisplayChild>>>>,
}

impl Grid {
    /// Create a new list widget. Closure will be called every time when a new child
    /// needs to be constructed.
    pub fn new<W: Widget<DisplayChild> + 'static>(closure: impl Fn() -> W + 'static) -> Self {
        Grid {
            closure: Box::new(move || Box::new(closure())),
            children: Vec::new(),
        }
    }

    /// When the widget is created or the data changes, create or remove children as needed
    ///
    /// Returns `true` if children were added or removed.
    fn update_child_count(&mut self, data: &impl ListIter<DisplayChild>, _env: &Env) -> bool {
        let len = self.children.len();
        match len.cmp(&data.data_len()) {
            Ordering::Greater => self.children.truncate(data.data_len()),
            Ordering::Less => data.for_each(|_, i| {
                if i >= len {
                    let child = WidgetPod::new((self.closure)());
                    self.children.push(child);
                }
            }),
            Ordering::Equal => (),
        }
        len != data.data_len()
    }
}

/// This iterator enables writing List widget for any `Data`.
pub trait ListIter<T>: Data {
    /// Iterate over each data child.
    fn for_each(&self, cb: impl FnMut(&T, usize));

    /// Iterate over each data child. Keep track of changed data and update self.
    fn for_each_mut(&mut self, cb: impl FnMut(&mut T, usize));

    /// Return data length.
    fn data_len(&self) -> usize;
}

impl<T: Data> ListIter<T> for Arc<Vec<T>> {
    fn for_each(&self, mut cb: impl FnMut(&T, usize)) {
        for (i, item) in self.iter().enumerate() {
            cb(item, i);
        }
    }

    fn for_each_mut(&mut self, mut cb: impl FnMut(&mut T, usize)) {
        let mut new_data = Vec::with_capacity(self.data_len());
        let mut any_changed = false;

        for (i, item) in self.iter().enumerate() {
            let mut d = item.to_owned();
            cb(&mut d, i);

            if !any_changed && !item.same(&d) {
                any_changed = true;
            }
            new_data.push(d);
        }

        if any_changed {
            *self = Arc::new(new_data);
        }
    }

    fn data_len(&self) -> usize {
        self.len()
    }
}

impl<T1: Data, T: Data> ListIter<(T1, T)> for (T1, Arc<Vec<T>>) {
    fn for_each(&self, mut cb: impl FnMut(&(T1, T), usize)) {
        for (i, item) in self.1.iter().enumerate() {
            let d = (self.0.clone(), item.to_owned());
            cb(&d, i);
        }
    }

    fn for_each_mut(&mut self, mut cb: impl FnMut(&mut (T1, T), usize)) {
        let shared = self.0.to_owned();
        let mut new_data = Vec::with_capacity(self.1.len());
        let mut any_shared_changed = false;
        let mut any_el_changed = false;

        for (i, item) in self.1.iter().enumerate() {
            let mut d = (self.0.clone(), item.to_owned());
            cb(&mut d, i);

            if !any_shared_changed && !shared.same(&d.0) {
                any_shared_changed = true;
            }
            if any_shared_changed {
                self.0 = d.0;
            }
            if !any_el_changed && !item.same(&d.1) {
                any_el_changed = true;
            }
            new_data.push(d.1);
        }

        if any_el_changed {
            self.1 = Arc::new(new_data);
        }
    }

    fn data_len(&self) -> usize {
        self.1.len()
    }
}

impl Widget<DisplayViewGrid> for Grid {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut DisplayViewGrid, env: &Env) {
        let mut children = self.children.iter_mut();
        data.children.for_each_mut(|child_data, _| {
            if let Some(child) = children.next() {
                child.event(ctx, event, child_data, env);
            }
        });
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &DisplayViewGrid,
        env: &Env,
    ) {
        if let LifeCycle::WidgetAdded = event {
            if self.update_child_count(&data.children, env) {
                ctx.children_changed();
            }
        }

        let mut children = self.children.iter_mut();
        data.children.for_each(|child_data, _| {
            if let Some(child) = children.next() {
                child.lifecycle(ctx, event, child_data, env);
            }
        });
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        _old_data: &DisplayViewGrid,
        data: &DisplayViewGrid,
        env: &Env,
    ) {
        // we send update to children first, before adding or removing children;
        // this way we avoid sending update to newly added children, at the cost
        // of potentially updating children that are going to be removed.
        let mut children = self.children.iter_mut();
        data.children.for_each(|child_data, _| {
            if let Some(child) = children.next() {
                child.update(ctx, child_data, env);
            }
        });

        if self.update_child_count(&data.children, env) {
            ctx.children_changed();
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &DisplayViewGrid,
        env: &Env,
    ) -> Size {
        let mut width: f64 = 0.0;
        let mut row_height = 0.0;
        let mut y = 0.0;
        let mut x = 0.0;

        let mut paint_rect = Rect::ZERO;
        let mut children = self.children.iter_mut();
        let cols = data.columns;
        data.children.for_each(|child_data, i| {
            if i % cols == 0 {
                y += row_height;
                width = width.max(x);
                row_height = bc.min().height;
                x = 0.0;
            }

            let child = match children.next() {
                Some(child) => child,
                None => {
                    return;
                }
            };
            let child_bc = BoxConstraints::new(
                Size::new(bc.min().width, 0.0),
                Size::new(bc.max().width, std::f64::INFINITY),
            );
            let child_size = child.layout(ctx, &child_bc, child_data, env);
            let rect = Rect::from_origin_size(Point::new(x, y), child_size);
            child.set_layout_rect(rect);
            paint_rect = paint_rect.union(child.paint_rect());

            x += child_size.width;
            row_height = row_height.max(child_size.height);
        });

        if row_height > bc.min().height {
            y += row_height;
        }
        width = width.max(bc.min().width);
        y = y.max(bc.min().height);

        let my_size = bc.constrain(Size::new(width, y));
        let insets = paint_rect - Rect::ZERO.with_size(my_size);
        ctx.set_paint_insets(insets);
        my_size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &DisplayViewGrid, env: &Env) {
        let mut children = self.children.iter_mut();
        data.children.for_each(|child_data, _| {
            if let Some(child) = children.next() {
                child.paint_with_offset(ctx, child_data, env);
            }
        });
    }
}