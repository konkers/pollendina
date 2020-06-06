use druid::kurbo::{Point, Rect, Size};

use druid::{
    BoxConstraints, Data, Env, Event, EventCtx, LayoutCtx, LifeCycle, LifeCycleCtx, PaintCtx,
    UpdateCtx, Widget, WidgetPod,
};

pub struct Stack<T> {
    children: Vec<WidgetPod<T, Box<dyn Widget<T>>>>,
}

impl<T> Stack<T> {
    pub fn new() -> Self {
        Stack {
            children: Vec::new(),
        }
    }

    pub fn with_child(mut self, child: impl Widget<T> + 'static) -> Self {
        self.children.push(WidgetPod::new(Box::new(child)));
        self
    }
}

impl<T: Data> Widget<T> for Stack<T> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        for child in &mut self.children {
            child.event(ctx, event, data, env);
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &T, env: &Env) {
        for child in &mut self.children {
            child.lifecycle(ctx, event, data, env);
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &T, data: &T, env: &Env) {
        for child in &mut self.children {
            child.update(ctx, data, env);
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &T, env: &Env) -> Size {
        bc.debug_check("Stack");
        let mut size = bc.min();
        for child in &mut self.children {
            let child_size = child.layout(ctx, bc, data, env);
            size.width = size.width.max(child_size.width);
            size.height = size.height.max(child_size.height);
            let rect = Rect::from_origin_size(Point::new(0., 0.), child_size);
            child.set_layout_rect(ctx, data, env, rect);
        }

        bc.constrain(size)
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        for child in &mut self.children {
            child.paint(ctx, data, env);
        }
    }
}
