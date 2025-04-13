//! An [imgui] rendering backend to integrate with the [sdl3 renderer][sdl3::render]

use imgui::internal::RawWrapper as _;
use std::{
    error::Error,
    ffi::{c_float, c_int, c_void},
    fmt::Display,
    mem::offset_of,
};

type RenderResult = std::result::Result<(), RenderError>;

/// A wrapper around various [sdl3] error types
#[derive(Debug)]
pub enum RenderError {
    UpdateTexture(sdl3::render::UpdateTextureError),
    TextureValue(sdl3::render::TextureValueError),
    GenericSDL(sdl3::Error),
}

impl From<sdl3::render::UpdateTextureError> for RenderError {
    fn from(value: sdl3::render::UpdateTextureError) -> Self {
        Self::UpdateTexture(value)
    }
}

impl From<sdl3::render::TextureValueError> for RenderError {
    fn from(value: sdl3::render::TextureValueError) -> Self {
        Self::TextureValue(value)
    }
}

impl From<sdl3::Error> for RenderError {
    fn from(value: sdl3::Error) -> Self {
        Self::GenericSDL(value)
    }
}

impl Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::UpdateTexture(e) => {
                write!(f, "{}", e)
            }
            Self::TextureValue(e) => {
                write!(f, "{}", e)
            }
            Self::GenericSDL(e) => {
                write!(f, "{}", e)
            }
        }
    }
}

impl Error for RenderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self {
            Self::UpdateTexture(e) => Some(e),
            Self::TextureValue(e) => Some(e),
            Self::GenericSDL(e) => Some(e),
        }
    }
}

/// Represents the context for the renderer
pub struct Renderer<'a> {
    texture_map: imgui::Textures<sdl3::render::Texture<'a>>,
    color_buffer: Vec<sdl3_sys::pixels::SDL_FColor>,
}

impl<'a> Renderer<'a> {
    /// Constructs a new [Renderer]
    ///
    /// # Examples
    /// Make sure to call after setting the imgui font.
    /// ```
    /// let sdl_context = sdl3::init().unwrap();
    /// let video_subsystem = sdl_context.video().unwrap();
    ///
    /// let window = video_subsystem
    ///     .window("rust-sdl3 example", 800, 600)
    ///     .position_centered()
    ///     .resizable()
    ///     .high_pixel_density()
    ///     .build()
    ///     .unwrap();
    /// let mut canvas = window.into_canvas();
    /// let texture_creator = canvas.texture_creator();
    ///
    /// let mut imgui_context = imgui::Context::create();
    /// imgui_context.set_ini_filename(None);
    ///
    /// imgui_context.fonts().add_font(&[imgui::FontSource::DefaultFontData { config: None, }]);
    ///
    /// let mut renderer = imgui_sdl3_renderer::Renderer::new(&texture_creator, &mut imgui_context).unwrap();
    /// ```
    pub fn new(
        texture_creator: &'a sdl3::render::TextureCreator<impl std::any::Any>,
        imgui_context: &mut imgui::Context,
    ) -> Result<Self, RenderError> {
        let mut texture_map = imgui::Textures::new();
        Self::prepare_font_atlas(texture_creator, imgui_context, &mut texture_map)?;

        imgui_context.set_renderer_name(Some(format!(
            "imgui-rs-sdl3-renderer {}",
            env!("CARGO_PKG_VERSION")
        )));

        imgui_context
            .io_mut()
            .backend_flags
            .insert(imgui::BackendFlags::RENDERER_HAS_VTX_OFFSET);

        Ok(Self {
            texture_map,
            color_buffer: Vec::new(),
        })
    }

    /// Renders the `draw_data` to the `canvas`
    ///
    /// <div class="warning">
    ///
    /// The `canvas` must be the canvas that owns the [TextureCreator] that was passed to
    /// [Self::new] and must be the same canvas on each call
    ///
    /// </div>
    ///
    /// # Examples
    /// ```ignore
    /// /* ... */
    /// let mut canvas = window.into_canvas();
    /// let texture_creator = canvas.texture_creator();
    ///
    /// /* ... */
    /// let mut renderer = imgui_sdl3_renderer::Renderer::new(&texture_creator, &mut imgui_context).unwrap();
    ///
    /// 'main loop {
    /// canvas.clear();
    /// /* ... */
    /// let ui = imgui_context.new_frame();
    /// ui.show_demo_window(&mut true);
    /// renderer.render(imgui_context.render(), &mut canvas).unwrap();
    /// /* ... */
    /// canvas.present();
    /// }
    /// ```
    pub fn render(
        &mut self,
        draw_data: &imgui::DrawData,
        canvas: &mut sdl3::render::Canvas<impl sdl3::render::RenderTarget>,
    ) -> RenderResult {
        struct CanvasBackup {
            viewport: sdl3::rect::Rect,
            clip: sdl3::render::ClippingRect,
        }

        let backup = CanvasBackup {
            viewport: canvas.viewport(),
            clip: canvas.clip_rect(),
        };

        Self::set_up_render_state(canvas);

        // Framebuffer scaling for HiDPI support
        let fb_width = draw_data.display_size[0] * draw_data.framebuffer_scale[0];
        let fb_height = draw_data.display_size[1] * draw_data.framebuffer_scale[1];

        // Don't need to render if minimised
        if fb_width == 0_f32 || fb_height == 0_f32 {
            return Ok(());
        }

        for draw_list in draw_data.draw_lists() {
            for command in draw_list.commands() {
                match command {
                    imgui::DrawCmd::Elements { count, cmd_params } => {
                        Self::render_elements(
                            &self.texture_map,
                            &mut self.color_buffer,
                            canvas,
                            draw_list.vtx_buffer(),
                            draw_list.idx_buffer(),
                            count,
                            &cmd_params,
                            &draw_data.display_pos,
                            &draw_data.framebuffer_scale,
                            (fb_width, fb_height),
                        )?;
                    }
                    imgui::DrawCmd::ResetRenderState => {
                        Self::set_up_render_state(canvas);
                    }
                    imgui::DrawCmd::RawCallback { callback, raw_cmd } => unsafe {
                        callback(draw_list.raw(), raw_cmd);
                    },
                }
            }
        }

        canvas.set_viewport(backup.viewport);
        canvas.set_clip_rect(backup.clip);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn render_elements(
        texture_map: &imgui::Textures<sdl3::render::Texture<'a>>,
        color_buffer: &mut Vec<sdl3_sys::pixels::SDL_FColor>,
        canvas: &mut sdl3::render::Canvas<impl sdl3::render::RenderTarget>,
        vertex_buffer: &[imgui::DrawVert],
        index_buffer: &[imgui::DrawIdx],
        elem_count: usize,
        elem_params: &imgui::DrawCmdParams,
        pos: &[f32; 2],
        scale: &[f32; 2],
        fb_size: (f32, f32),
    ) -> RenderResult {
        let imgui::DrawCmdParams {
            clip_rect,
            texture_id,
            vtx_offset,
            idx_offset,
        } = elem_params;

        let clip_min = (
            (clip_rect[0] - pos[0]) * scale[0],
            (clip_rect[1] - pos[1]) * scale[1],
        );
        let clip_max = (
            (clip_rect[2] - pos[0]) * scale[0],
            (clip_rect[3] - pos[1]) * scale[1],
        );
        if clip_min.0 >= fb_size.0
            || clip_min.1 >= fb_size.1
            || clip_max.0 < 0.0
            || clip_max.1 < 0.0
        {
            return Ok(());
        }

        let rect = sdl3::rect::Rect::new(
            clip_min.0 as i32,
            clip_min.1 as i32,
            (clip_max.0 - clip_min.0) as u32,
            (clip_max.1 - clip_min.1) as u32,
        );
        canvas.set_clip_rect(rect);

        let texture = texture_map.get(*texture_id);
        Self::render_raw_geometry(
            canvas,
            color_buffer,
            texture,
            &vertex_buffer[*vtx_offset..],
            &index_buffer[*idx_offset..idx_offset + elem_count],
        )
    }

    fn render_raw_geometry(
        canvas: &mut sdl3::render::Canvas<impl sdl3::render::RenderTarget>,
        color_buffer: &mut Vec<sdl3_sys::pixels::SDL_FColor>,
        texture: Option<&sdl3::render::Texture>,
        vertices: &[imgui::DrawVert],
        indices: &[imgui::DrawIdx],
    ) -> RenderResult {
        let vert_stride = size_of::<imgui::DrawVert>() as c_int;
        color_buffer.clear();
        // Normalize colours to SDL_Fcolor format 
        color_buffer.extend(vertices.iter().map(|vert| sdl3_sys::pixels::SDL_FColor {
            r: vert.col[0] as f32 / 255_f32,
            g: vert.col[1] as f32 / 255_f32,
            b: vert.col[2] as f32 / 255_f32,
            a: vert.col[3] as f32 / 255_f32,
        }));

        let renderer = canvas.raw();
        let texture = texture.map_or(std::ptr::null_mut(), |texture| texture.raw());

        let xy = unsafe {
            vertices.as_ptr().byte_add(offset_of!(imgui::DrawVert, pos)) as *const c_float
        };
        let uv = unsafe {
            vertices.as_ptr().byte_add(offset_of!(imgui::DrawVert, uv)) as *const c_float
        };
        let idx = indices.as_ptr() as *const c_void;
        let colors = color_buffer.as_ptr();

        unsafe {
            sdl3_sys::render::SDL_RenderGeometryRaw(
                renderer,
                texture,
                xy,
                vert_stride,
                colors,
                size_of::<sdl3_sys::pixels::SDL_FColor>() as c_int,
                uv,
                vert_stride,
                vertices.len() as c_int,
                idx,
                indices.len() as c_int,
                size_of::<imgui::DrawIdx>() as c_int,
            )
        }
        .then_some(())
        .ok_or_else(|| sdl3::get_error().into())
    }

    fn set_up_render_state(canvas: &mut sdl3::render::Canvas<impl sdl3::render::RenderTarget>) {
        canvas.set_clip_rect(None);
        canvas.set_viewport(None);
    }

    fn prepare_font_atlas(
        creator: &'a sdl3::render::TextureCreator<impl std::any::Any>,
        imgui_context: &mut imgui::Context,
        texture_map: &mut imgui::Textures<sdl3::render::Texture<'a>>,
    ) -> RenderResult {
        let font_atlas = imgui_context.fonts().build_rgba32_texture();
        let rgba32_format: sdl3::pixels::PixelFormat =
            sdl3_sys::pixels::SDL_PixelFormat::RGBA32.try_into()?;
        let mut font_texture =
            creator.create_texture_static(rgba32_format, font_atlas.width, font_atlas.height)?;

        font_texture.update(
            None,
            font_atlas.data,
            rgba32_format.byte_size_of_pixels(font_atlas.width as usize),
        )?;

        font_texture.set_blend_mode(sdl3::render::BlendMode::Blend);
        font_texture.set_scale_mode(sdl3::render::ScaleMode::Linear);

        let id = texture_map.insert(font_texture);
        imgui_context.fonts().tex_id = id;
        Ok(())
    }
}

