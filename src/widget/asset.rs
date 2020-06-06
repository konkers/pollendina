use std::sync::Arc;

use druid::{
    piet::InterpolationMode, widget::FillStrat, BoxConstraints, Data, Env, Event, EventCtx,
    LayoutCtx, LifeCycle, LifeCycleCtx, PaintCtx, Rect, RenderContext, Size, UpdateCtx, Widget,
};

use crate::assets::{image::ImageData, IMAGES};

pub struct Asset {
    image: Option<Arc<ImageData>>,
}

impl Asset {
    pub fn new() -> Self {
        Asset { image: None }
    }

    fn update_image(&mut self, id: &String) {
        IMAGES.with(|images| {
            self.image = images.borrow().get(&id);
        });
    }
}

impl Widget<String> for Asset {
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event, _data: &mut String, _env: &Env) {}

    fn lifecycle(&mut self, _ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &String, _env: &Env) {
        if let LifeCycle::WidgetAdded = event {
            self.update_image(data);
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &String, data: &String, _env: &Env) {
        if !old_data.same(&data) {
            self.update_image(data);
            ctx.request_layout();
        }
    }

    fn layout(
        &mut self,
        _layout_ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _data: &String,
        _env: &Env,
    ) -> Size {
        bc.debug_check("Asset");

        if let Some(i) = &self.image {
            let mut img_size = i.get_size();
            let x_scale = bc.max().width / img_size.width;
            let y_scale = bc.max().height / img_size.height;
            let scale = x_scale.min(y_scale);
            img_size.width *= scale;
            img_size.height *= scale;

            bc.constrain(img_size)
        } else {
            Size::ZERO
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, _data: &String, _env: &Env) {
        if let Some(i) = &self.image {
            let fill = FillStrat::default();
            let offset_matrix = fill.affine_to_fill(ctx.size(), i.get_size());

            if fill != FillStrat::Contain {
                let clip_rect = Rect::ZERO.with_size(ctx.size());
                ctx.clip(clip_rect);
            }
            i.to_piet(offset_matrix, ctx, InterpolationMode::Bilinear);
        }
    }
}
