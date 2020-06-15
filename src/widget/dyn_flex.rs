// Copyright 2018 The xi-editor Authors.
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

use druid::kurbo::common::FloatExt;
use druid::kurbo::{Point, Rect, Size};

use druid::{
    BoxConstraints, Data, Env, Event, EventCtx, KeyOrValue, LayoutCtx, LifeCycle, LifeCycleCtx,
    PaintCtx, UpdateCtx, Widget, WidgetPod,
};

use super::list_iter::ListIter;

pub trait DynFlexItem {
    fn flex_params(&self) -> DynFlexParams;
}

pub struct DynFlex<T> {
    direction: Axis,
    cross_alignment: CrossAxisAlignment,
    main_alignment: MainAxisAlignment,
    fill_major_axis: bool,
    fill_minor_axis: bool,
    children: Vec<ChildWidget<T>>,
    closure: Box<dyn Fn() -> Box<dyn Widget<T>>>,
}

struct ChildWidget<T> {
    widget: WidgetPod<T, Box<dyn Widget<T>>>,
    params: DynFlexParams,
}

/// A dummy widget we use to do spacing.
struct Spacer {
    axis: Axis,
    len: KeyOrValue<f64>,
}

#[derive(Copy, Clone, Default, PartialEq)]
pub struct DynFlexParams {
    flex: f64,
    alignment: Option<CrossAxisAlignment>,
}

#[derive(Clone, Copy)]
pub(crate) enum Axis {
    Horizontal,
    Vertical,
}

/// The alignment of the widgets on the container's cross (or minor) axis.
///
/// If a widget is smaller than the container on the minor axis, this determines
/// where it is positioned.
#[derive(Debug, Clone, Copy, PartialEq, Data)]
#[allow(dead_code)]
pub enum CrossAxisAlignment {
    /// Top or leading.
    ///
    /// In a vertical container, widgets are top aligned. In a horiziontal
    /// container, their leading edges are aligned.
    Start,
    /// Widgets are centered in the container.
    Center,
    /// Bottom or trailing.
    ///
    /// In a vertical container, widgets are bottom aligned. In a horiziontal
    /// container, their trailing edges are aligned.
    End,
}

/// Arrangement of children on the main axis.
///
/// If there is surplus space on the main axis after laying out children, this
/// enum represents how children are laid out in this space.
#[derive(Debug, Clone, Copy, PartialEq, Data)]
#[allow(dead_code)]
pub enum MainAxisAlignment {
    /// Top or leading.
    ///
    /// Children are aligned with the top or leading edge, without padding.
    Start,
    /// Children are centered, without padding.
    Center,
    /// Bottom or trailing.
    ///
    /// Children are aligned with the bottom or trailing edge, without padding.
    End,
    /// Extra space is divided evenly between each child.
    SpaceBetween,
    /// Extra space is divided evenly between each child, as well as at the ends.
    SpaceEvenly,
    /// Space between each child, with less at the start and end.
    ///
    /// This divides space such that each child is separated by `n` units,
    /// and the start and end have `n/2` units of padding.
    SpaceAround,
}

impl DynFlexParams {
    /// Create custom `FlexParams` with a specific `flex_factor` and an optional
    /// [`CrossAxisAlignment`].
    ///
    /// You likely only need to create these manually if you need to specify
    /// a custom alignment; if you only need to use a custom `flex_factor` you
    /// can pass an `f64` to any of the functions that take `FlexParams`.
    ///
    /// By default, the widget uses the alignment of its parent [`Flex`] container.
    ///
    ///
    /// [`Flex`]: struct.Flex.html
    /// [`CrossAxisAlignment`]: enum.CrossAxisAlignment.html
    #[allow(dead_code)]
    pub fn new(flex: f64, alignment: impl Into<Option<CrossAxisAlignment>>) -> Self {
        DynFlexParams {
            flex,
            alignment: alignment.into(),
        }
    }
}

impl<T> ChildWidget<T> {
    fn new(child: impl Widget<T> + 'static, params: DynFlexParams) -> Self {
        ChildWidget {
            widget: WidgetPod::new(Box::new(child)),
            params,
        }
    }
}

impl Axis {
    pub(crate) fn major(self, coords: Size) -> f64 {
        match self {
            Axis::Horizontal => coords.width,
            Axis::Vertical => coords.height,
        }
    }

    pub(crate) fn minor(self, coords: Size) -> f64 {
        match self {
            Axis::Horizontal => coords.height,
            Axis::Vertical => coords.width,
        }
    }

    pub(crate) fn pack(self, major: f64, minor: f64) -> (f64, f64) {
        match self {
            Axis::Horizontal => (major, minor),
            Axis::Vertical => (minor, major),
        }
    }

    /// Generate constraints with new values on the major axis.
    fn constraints(self, bc: &BoxConstraints, min_major: f64, major: f64) -> BoxConstraints {
        match self {
            Axis::Horizontal => BoxConstraints::new(
                Size::new(min_major, bc.min().height),
                Size::new(major, bc.max().height),
            ),
            Axis::Vertical => BoxConstraints::new(
                Size::new(bc.min().width, min_major),
                Size::new(bc.max().width, major),
            ),
        }
    }
}

impl<T: Data + DynFlexItem> DynFlex<T> {
    /// Create a new horizontal stack.
    ///
    /// The child widgets are laid out horizontally, from left to right.
    pub fn row<W: Widget<T> + 'static>(closure: impl Fn() -> W + 'static) -> Self {
        DynFlex {
            direction: Axis::Horizontal,
            closure: Box::new(move || Box::new(closure())),
            children: Vec::new(),
            cross_alignment: CrossAxisAlignment::Center,
            main_alignment: MainAxisAlignment::Start,
            fill_major_axis: false,
            fill_minor_axis: false,
        }
    }

    /// Create a new vertical stack.
    ///
    /// The child widgets are laid out vertically, from top to bottom.
    pub fn column<W: Widget<T> + 'static>(closure: impl Fn() -> W + 'static) -> Self {
        DynFlex {
            direction: Axis::Vertical,
            closure: Box::new(move || Box::new(closure())),
            children: Vec::new(),
            cross_alignment: CrossAxisAlignment::Center,
            main_alignment: MainAxisAlignment::Start,
            fill_major_axis: false,
            fill_minor_axis: false,
        }
    }

    /// When the widget is created or the data changes, create or remove children as needed
    ///
    /// Returns `true` if children were added or removed.
    fn update_child_count(&mut self, data: &impl ListIter<T>, _env: &Env) -> bool {
        let len = self.children.len();
        match len.cmp(&data.data_len()) {
            Ordering::Greater => self.children.truncate(data.data_len()),
            Ordering::Less => data.for_each(|_, i| {
                if i >= len {
                    let widget = (self.closure)();
                    let child = ChildWidget::new(widget, 0.0.into());

                    self.children.push(child);
                }
            }),
            Ordering::Equal => (),
        }
        len != data.data_len()
    }

    /// Builder-style method for specifying the childrens' [`CrossAxisAlignment`].
    ///
    /// [`CrossAxisAlignment`]: enum.CrossAxisAlignment.html
    #[allow(dead_code)]
    pub fn cross_axis_alignment(mut self, alignment: CrossAxisAlignment) -> Self {
        self.cross_alignment = alignment;
        self
    }

    /// Builder-style method for specifying the childrens' [`MainAxisAlignment`].
    ///
    /// [`MainAxisAlignment`]: enum.MainAxisAlignment.html
    #[allow(dead_code)]
    pub fn main_axis_alignment(mut self, alignment: MainAxisAlignment) -> Self {
        self.main_alignment = alignment;
        self
    }

    /// Builder-style method for setting whether the container must expand
    /// to fill the available space on its main axis.
    ///
    /// If any children have flex then this container will expand to fill all
    /// available space on its main axis; But if no children are flex,
    /// this flag determines whether or not the container should shrink to fit,
    /// or must expand to fill.
    ///
    /// If it expands, and there is extra space left over, that space is
    /// distributed in accordance with the [`MainAxisAlignment`].
    ///
    /// The default value is `false`.
    ///
    /// [`MainAxisAlignment`]: enum.MainAxisAlignment.html
    #[allow(dead_code)]
    pub fn must_fill_main_axis(mut self, fill: bool) -> Self {
        self.fill_major_axis = fill;
        self
    }

    pub fn must_fill_minor_axis(mut self, fill: bool) -> Self {
        self.fill_minor_axis = fill;
        self
    }
}

impl<C: Data + DynFlexItem, T: ListIter<C>> Widget<T> for DynFlex<C> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        let mut children = self.children.iter_mut();
        data.for_each_mut(|child_data, _| {
            if let Some(child) = children.next() {
                child.widget.event(ctx, event, child_data, env);
            }
        });
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &T, env: &Env) {
        if let LifeCycle::WidgetAdded = event {
            if self.update_child_count(data, env) {
                ctx.children_changed();
            }
        }

        let mut children = self.children.iter_mut();
        data.for_each(|child_data, _| {
            if let Some(child) = children.next() {
                child.widget.lifecycle(ctx, event, child_data, env);
                child.params = child_data.flex_params();
            }
        });
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &T, data: &T, env: &Env) {
        let mut children_changed = false;
        // we send update to children first, before adding or removing children;
        // this way we avoid sending update to newly added children, at the cost
        // of potentially updating children that are going to be removed.
        let mut children = self.children.iter_mut();
        data.for_each(|child_data, _| {
            if let Some(child) = children.next() {
                child.widget.update(ctx, child_data, env);
                let new_params = child_data.flex_params();
                if new_params != child.params {
                    child.params = new_params;
                    children_changed = true;
                }
            }
        });

        children_changed |= self.update_child_count(data, env);

        if children_changed {
            ctx.children_changed();
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &T, env: &Env) -> Size {
        bc.debug_check("Flex");
        // we loosen our constraints when passing to children.
        let loosened_bc = bc.loosen();

        // Vec of cloned child data;
        let mut children_data = Vec::new();
        data.for_each(|child_data, _| {
            children_data.push(child_data.clone());
        });
        // Measure non-flex children.
        let mut major_non_flex = 0.0;
        let mut minor = if self.fill_minor_axis {
            self.direction.minor(bc.max())
        } else {
            self.direction.minor(bc.min())
        };

        let mut children_data_iter = children_data.iter();
        for child in &mut self.children {
            if let Some(child_data) = children_data_iter.next() {
                if child.params.flex == 0.0 {
                    let child_bc = self
                        .direction
                        .constraints(&loosened_bc, 0., std::f64::INFINITY);
                    let child_size = child.widget.layout(ctx, &child_bc, child_data, env);

                    if child_size.width.is_infinite() {
                        log::warn!("A non-Flex child has an infinite width.");
                    }

                    if child_size.height.is_infinite() {
                        log::warn!("A non-Flex child has an infinite height.");
                    }

                    major_non_flex += self.direction.major(child_size).expand();
                    minor = minor.max(self.direction.minor(child_size).expand());
                    // Stash size.
                    let rect = Rect::from_origin_size(Point::ORIGIN, child_size);
                    child.widget.set_layout_rect(ctx, child_data, env, rect);
                }
            }
        }

        let total_major = self.direction.major(bc.max());
        let remaining = (total_major - major_non_flex).max(0.0);
        let mut remainder: f64 = 0.0;
        let flex_sum: f64 = self.children.iter().map(|child| child.params.flex).sum();
        let mut major_flex: f64 = 0.0;

        // Measure flex children.
        let mut children_data_iter = children_data.iter();
        for child in &mut self.children {
            if let Some(child_data) = children_data_iter.next() {
                if child.params.flex != 0.0 {
                    let desired_major = remaining * child.params.flex / flex_sum + remainder;
                    let actual_major = desired_major.round();
                    remainder = desired_major - actual_major;
                    let min_major = 0.0;

                    let child_bc =
                        self.direction
                            .constraints(&loosened_bc, min_major, actual_major);
                    let child_size = child.widget.layout(ctx, &child_bc, child_data, env);

                    major_flex += self.direction.major(child_size).expand();
                    minor = minor.max(self.direction.minor(child_size).expand());
                    // Stash size.
                    let rect = Rect::from_origin_size(Point::ORIGIN, child_size);
                    child.widget.set_layout_rect(ctx, child_data, env, rect);
                }
            }
        }

        // figure out if we have extra space on major axis, and if so how to use it
        let extra = if self.fill_major_axis {
            (remaining - major_flex).max(0.0)
        } else {
            // if we are *not* expected to fill our available space this usually
            // means we don't have any extra, unless dictated by our constraints.
            (self.direction.major(bc.min()) - (major_non_flex + major_flex)).max(0.0)
        };

        let mut spacing = Spacing::new(self.main_alignment, extra, self.children.len());
        // Finalize layout, assigning positions to each child.
        let mut major = spacing.next().unwrap_or(0.);
        let mut child_paint_rect = Rect::ZERO;
        let mut children_data_iter = children_data.iter();
        for child in &mut self.children {
            if let Some(child_data) = children_data_iter.next() {
                let rect = child.widget.layout_rect();
                let extra_minor = minor - self.direction.minor(rect.size());
                let alignment = child.params.alignment.unwrap_or(self.cross_alignment);
                let align_minor = alignment.align(extra_minor);
                let pos: Point = self.direction.pack(major, align_minor).into();

                child
                    .widget
                    .set_layout_rect(ctx, child_data, env, rect.with_origin(pos));
                child_paint_rect = child_paint_rect.union(child.widget.paint_rect());
                major += self.direction.major(rect.size()).expand();
                major += spacing.next().unwrap_or(0.);
            }
        }

        if flex_sum > 0.0 && total_major.is_infinite() {
            log::warn!("A child of Flex is flex, but Flex is unbounded.")
        }

        if flex_sum > 0.0 {
            major = total_major;
        }

        let my_size: Size = self.direction.pack(major, minor).into();

        // if we don't have to fill the main axis, we loosen that axis before constraining
        let my_size = if !self.fill_major_axis {
            let max_major = self.direction.major(bc.max());
            self.direction
                .constraints(bc, 0.0, max_major)
                .constrain(my_size)
        } else {
            bc.constrain(my_size)
        };

        let my_bounds = Rect::ZERO.with_size(my_size);
        let insets = child_paint_rect - my_bounds;
        ctx.set_paint_insets(insets);
        my_size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        let mut children = self.children.iter_mut();
        data.for_each(|child_data, _| {
            if let Some(child) = children.next() {
                child.widget.paint(ctx, child_data, env);
            }
        });
    }
}

impl CrossAxisAlignment {
    /// Given the difference between the size of the container and the size
    /// of the child (on their minor axis) return the necessary offset for
    /// this alignment.
    fn align(self, val: f64) -> f64 {
        match self {
            CrossAxisAlignment::Start => 0.0,
            CrossAxisAlignment::Center => (val / 2.0).round(),
            CrossAxisAlignment::End => val,
        }
    }
}

struct Spacing {
    alignment: MainAxisAlignment,
    extra: f64,
    n_children: usize,
    index: usize,
    equal_space: f64,
    remainder: f64,
}

impl Spacing {
    /// Given the provided extra space and children count,
    /// this returns an iterator of `f64` spacing,
    /// where the first element is the spacing before any children
    /// and all subsequent elements are the spacing after children.
    fn new(alignment: MainAxisAlignment, extra: f64, n_children: usize) -> Spacing {
        let extra = if extra.is_finite() { extra } else { 0. };
        let equal_space = if n_children > 0 {
            match alignment {
                MainAxisAlignment::Center => extra / 2.,
                MainAxisAlignment::SpaceBetween => extra / (n_children - 1).max(1) as f64,
                MainAxisAlignment::SpaceEvenly => extra / (n_children + 1) as f64,
                MainAxisAlignment::SpaceAround => extra / (2 * n_children) as f64,
                _ => 0.,
            }
        } else {
            0.
        };
        Spacing {
            alignment,
            extra,
            n_children,
            index: 0,
            equal_space,
            remainder: 0.,
        }
    }

    fn next_space(&mut self) -> f64 {
        let desired_space = self.equal_space + self.remainder;
        let actual_space = desired_space.round();
        self.remainder = desired_space - actual_space;
        actual_space
    }
}

impl Iterator for Spacing {
    type Item = f64;

    fn next(&mut self) -> Option<f64> {
        if self.index > self.n_children {
            return None;
        }
        let result = {
            if self.n_children == 0 {
                self.extra
            } else {
                #[allow(clippy::match_bool)]
                match self.alignment {
                    MainAxisAlignment::Start => match self.index == self.n_children {
                        true => self.extra,
                        false => 0.,
                    },
                    MainAxisAlignment::End => match self.index == 0 {
                        true => self.extra,
                        false => 0.,
                    },
                    MainAxisAlignment::Center => match self.index {
                        0 => self.next_space(),
                        i if i == self.n_children => self.next_space(),
                        _ => 0.,
                    },
                    MainAxisAlignment::SpaceBetween => match self.index {
                        0 => 0.,
                        i if i != self.n_children => self.next_space(),
                        _ => match self.n_children {
                            1 => self.next_space(),
                            _ => 0.,
                        },
                    },
                    MainAxisAlignment::SpaceEvenly => self.next_space(),
                    MainAxisAlignment::SpaceAround => {
                        if self.index == 0 || self.index == self.n_children {
                            self.next_space()
                        } else {
                            self.next_space() + self.next_space()
                        }
                    }
                }
            }
        };
        self.index += 1;
        Some(result)
    }
}

impl<T: Data + DynFlexItem> Widget<T> for Spacer {
    fn event(&mut self, _: &mut EventCtx, _: &Event, _: &mut T, _: &Env) {}
    fn lifecycle(&mut self, _: &mut LifeCycleCtx, _: &LifeCycle, _: &T, _: &Env) {}
    fn update(&mut self, _: &mut UpdateCtx, _: &T, _: &T, _: &Env) {}
    fn layout(&mut self, _: &mut LayoutCtx, _: &BoxConstraints, _: &T, env: &Env) -> Size {
        let major = self.len.resolve(env);
        self.axis.pack(major, 0.0).into()
    }
    fn paint(&mut self, _: &mut PaintCtx, _: &T, _: &Env) {}
}

impl From<f64> for DynFlexParams {
    fn from(flex: f64) -> DynFlexParams {
        DynFlexParams {
            flex,
            alignment: None,
        }
    }
}

// Blanket implementation of `DynFlexItem` for use with ListIters with shared data.
impl<T1: Data, T: Data + DynFlexItem> DynFlexItem for (T1, T) {
    fn flex_params(&self) -> DynFlexParams {
        self.1.flex_params()
    }
}
