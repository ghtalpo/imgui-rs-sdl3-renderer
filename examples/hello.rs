use sdl3::{
    event::Event, pixels::Color, render::FRect
};

use imgui_sdl3_support::SdlPlatform;

fn main() {
    let mut sdl_context = sdl3::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("rust-sdl3 example", 800, 600)
        .opengl()
        .position_centered()
        .resizable()
        .high_pixel_density()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas();
    let texture_creator = canvas.texture_creator();

    let mut imgui_context = imgui::Context::create();
    imgui_context.set_ini_filename(None);

    imgui_context
        .fonts()
        .add_font(&[imgui::FontSource::DefaultFontData { config: None }]);

    let mut platform = SdlPlatform::new(&mut imgui_context);
    let mut renderer =
        imgui_sdl3_renderer::Renderer::new(&texture_creator, &mut imgui_context).unwrap();

    let mut event_pump = sdl_context.event_pump().unwrap();

    'main: loop {
        for event in event_pump.poll_iter() {
            /* pass all events to imgui platfrom */
            platform.handle_event(&mut imgui_context, &event);

            if let Event::Quit { .. } = event {
                break 'main;
            }
        }

        platform.prepare_frame(&mut sdl_context, &mut imgui_context, &canvas.window(), &event_pump);

        canvas.clear();

        let color_ = canvas.draw_color();
        canvas.set_draw_color(Color::GREEN);
        canvas.draw_rect(FRect::new(0.0, 0.0, 100.0, 100.0)).unwrap();
        canvas.set_draw_color(color_);

        /* ... */
        let ui = imgui_context.new_frame();
        ui.show_demo_window(&mut true);
        renderer
            .render(imgui_context.render(), &mut canvas)
            .unwrap();
        /* ... */
        canvas.present();
    }
}
