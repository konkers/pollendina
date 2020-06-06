use std::cmp::Ordering;

use druid::kurbo::{Point, Rect, Size};

use druid::{
    BoxConstraints, Data, Env, Event, EventCtx, LayoutCtx, LifeCycle, LifeCycleCtx, PaintCtx,
    UpdateCtx, Widget, WidgetPod,
};

use super::list_iter::ListIter;

pub trait Star {
    fn pos(&self) -> (f64, f64);
    fn radius(&self) -> f64;
}

pub trait Field {
    fn size(&self) -> (f64, f64);
}

pub struct Constellation<T: Data + Star> {
    scale: f64,
    children: Vec<ChildWidget<T>>,
    closure: Box<dyn Fn() -> Box<dyn Widget<T>>>,
}

struct ChildWidget<T> {
    widget: WidgetPod<T, Box<dyn Widget<T>>>,
    pos: (f64, f64),
    radius: f64,
}

impl<T: Star> ChildWidget<T> {
    fn new(child: impl Widget<T> + 'static, data: &T) -> Self {
        ChildWidget {
            widget: WidgetPod::new(Box::new(child)),
            pos: data.pos(),
            radius: data.radius(),
        }
    }
}

impl<T: Data + Star> Constellation<T> {
    pub fn new<W: Widget<T> + 'static>(closure: impl Fn() -> W + 'static) -> Self {
        Constellation {
            scale: 1.0,
            children: Vec::new(),
            closure: Box::new(move || Box::new(closure())),
        }
    }

    fn update_child_count(&mut self, data: &impl ListIter<T>, _env: &Env) -> bool {
        let len = self.children.len();
        match len.cmp(&data.data_len()) {
            Ordering::Greater => self.children.truncate(data.data_len()),
            Ordering::Less => data.for_each(|data, i| {
                if i >= len {
                    let widget = (self.closure)();
                    let child = ChildWidget::new(widget, &data);
                    self.children.push(child);
                }
            }),
            Ordering::Equal => (),
        }
        len != data.data_len()
    }
}

impl<C: Data + Star, T: ListIter<C> + Field> Widget<T> for Constellation<C> {
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
                let new_pos = child_data.pos();
                let new_radius = child_data.radius();
                if new_pos != child.pos || new_radius != child.radius {
                    child.pos = new_pos;
                    child.radius = new_radius;
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
        bc.debug_check("Constellation");

        // Scale the field to the available space
        let mut field_size: Size = data.size().into();
        let x_scale = bc.max().width / field_size.width;
        let y_scale = bc.max().height / field_size.height;
        let scale = x_scale.min(y_scale);
        field_size.width *= scale;
        field_size.height *= scale;
        self.scale = scale;
        let size = bc.constrain(field_size);

        let mut children = self.children.iter_mut();
        data.for_each(|child_data, _| {
            if let Some(child) = children.next() {
                // Scale the can constrain the child widget by it's radius.
                let child_bc = BoxConstraints::new(
                    bc.min(),
                    Size::new(child.radius * scale * 2., child.radius * scale * 2.),
                );
                let child_size = child.widget.layout(ctx, &child_bc, child_data, env);

                // Now center the widget on its position.
                let child_pos = Point::new(child.pos.0 * scale, child.pos.1 * scale);
                let rect = Rect::from_center_size(child_pos, child_size);
                child.widget.set_layout_rect(ctx, child_data, env, rect);
            }
        });
        size
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
