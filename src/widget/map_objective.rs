use druid::{
    kurbo::{Circle, Size},
    BoxConstraints, Color, Data, Env, Event, EventCtx, LayoutCtx, LifeCycle, LifeCycleCtx,
    PaintCtx, RenderContext, UpdateCtx, Widget,
};

use crate::engine::ObjectiveState;

pub struct MapObjective {
    radius: f64,
}

impl MapObjective {
    pub fn new() -> MapObjective {
        MapObjective { radius: 0. }
    }
}

impl Widget<ObjectiveState> for MapObjective {
    fn event(
        &mut self,
        _ctx: &mut EventCtx,
        _event: &Event,
        _data: &mut ObjectiveState,
        _env: &Env,
    ) {
    }

    fn lifecycle(
        &mut self,
        _ctx: &mut LifeCycleCtx,
        _event: &LifeCycle,
        _data: &ObjectiveState,
        _env: &Env,
    ) {
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        old_data: &ObjectiveState,
        data: &ObjectiveState,
        _env: &Env,
    ) {
        if !old_data.same(data) {
            ctx.children_changed();
        }
    }

    fn layout(
        &mut self,
        _ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _data: &ObjectiveState,
        _env: &Env,
    ) -> Size {
        // Set our radius to the maximum circle that will fit in our constraints.
        let width = bc.max().width;
        let height = bc.max().height;

        let d = width.min(height);
        self.radius = d / 2.;

        bc.constrain((d, d))
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &ObjectiveState, _env: &Env) {
        let bg_color = Color::rgb8(0x00, 0x00, 0x00);
        let outline_color = Color::rgb8(0xff, 0xff, 0xff);

        let unlocked_color = Color::rgb8(0x00, 0xff, 0x00);
        let complete_color = Color::rgb8(0x00, 0x88, 0xcc);
        let glitch_locked_color = Color::rgb8(0xff, 0xff, 0x00);
        let locked_color = Color::rgb8(0x44, 0x44, 0x44);

        let r = self.radius;

        let bg_radius = r;
        let outline_radius = r * 0.8;
        let outline_width = r * 0.2;
        let inner_radius = r * 0.6;

        let inner_color = match data {
            ObjectiveState::Disabled => return,
            ObjectiveState::Complete => &complete_color,
            ObjectiveState::Locked => &locked_color,
            ObjectiveState::GlitchLocked => &glitch_locked_color,
            ObjectiveState::Unlocked => &unlocked_color,
        };

        let pos = (r, r);
        let inner_circle = Circle::new(pos, inner_radius);
        let bg_circle = Circle::new(pos, bg_radius);
        let outline_circle = Circle::new(pos, outline_radius);

        ctx.fill(bg_circle, &bg_color);
        ctx.fill(inner_circle, inner_color);
        ctx.stroke(outline_circle, &outline_color, outline_width);
    }
}
