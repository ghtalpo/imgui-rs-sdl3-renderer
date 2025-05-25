use sdl3::{
    event::Event,
};

fn main() {
    let sdl_context = sdl3::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    println!("before create window");

    let window = video_subsystem
        .window("rust-sdl3 example", 800, 600)
        .opengl()
        .position_centered()
        .resizable()
        // .high_pixel_density()
        .build()
        .unwrap();

    // let _ = window.gl_set_context_to_current();

    let mut canvas = window.into_canvas();
    let texture_creator = canvas.texture_creator();

    println!("before create imgui_context");
    
    let mut imgui_context = imgui::Context::create();
    imgui_context.set_ini_filename(None);

    println!("before create imgui_context add_font");

    imgui_context
        .fonts()
        .add_font(&[imgui::FontSource::DefaultFontData { config: None }]);

    println!("before create imgui_context renderer");

    let mut renderer =
        imgui_sdl3_renderer::Renderer::new(&texture_creator, &mut imgui_context).unwrap();

    let mut event_pump = sdl_context.event_pump().unwrap();

    println!("before entering loop");

    'main: loop {
        println!("begin loop");
        for event in event_pump.poll_iter() {
            /* pass all events to imgui platfrom */
            // platform.handle_event(&mut imgui, &event);

            if let Event::Quit { .. } = event {
                break 'main;
            }
        }

        println!("mid0 loop");

        canvas.clear();
        /* ... */
        let ui = imgui_context.new_frame();
        ui.show_demo_window(&mut true);
        renderer
            .render(imgui_context.render(), &mut canvas)
            .unwrap();
        /* ... */
        canvas.present();
        println!("begin end");
    }
}
