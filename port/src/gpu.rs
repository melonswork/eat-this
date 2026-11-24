//! Extras: the window's picture. The renderer still makes the finished RGB frame; this puts it on
//! the window through OpenGL with the chosen shaders, or through SDL's renderer (no shaders) when
//! there is no OpenGL 3.3. With the default settings both show the frame's plain pixels.

use glow::HasContext;
use sdl2::pixels::PixelFormatEnum;
use sdl2::video::Window;
use std::error::Error;

/// `Look::upscale`.
#[allow(dead_code)]
pub const PLAIN: i32 = 0;
pub const SHARP: i32 = 1;
pub const SCALE4X: i32 = 2;
pub const XBR: i32 = 3;

#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Look {
    pub upscale: i32,
    pub crt: bool,
    /// Whole multiples of the picture's size only.
    pub integer: bool,
}

/// A frame: `rgb` is w x h, shown `size` big in the middle of a `frame`-sized picture (both in the
/// original's pixels).
pub struct Pic<'a> {
    pub rgb: &'a [u8],
    pub w: i32,
    pub h: i32,
    pub size: (i32, i32),
    pub frame: (i32, i32),
}

const VS: &str = "#version 330 core
uniform vec4 rect; // in clip space: x0 y0 x1 y1
out vec2 uv;
void main() {
    vec2 c = vec2(gl_VertexID & 1, gl_VertexID >> 1);
    uv = vec2(c.x, 1.0 - c.y);
    gl_Position = vec4(mix(rect.xy, rect.zw, c), 0.0, 1.0);
}";

const PLAIN_FS: &str = "#version 330 core
uniform sampler2D tex; in vec2 uv; out vec4 o;
void main() { o = texture(tex, uv); }";

const SHARP_FS: &str = "#version 330 core
uniform sampler2D tex; uniform vec2 src; uniform vec2 dst; in vec2 uv; out vec4 o;
void main() {
    vec2 p = uv * src, k = max(dst / src, vec2(1.0));
    vec2 c = fract(p) - 0.5, r = 0.5 - 0.5 / k;
    o = texture(tex, (floor(p) + (c - clamp(c, -r, r)) * k + 0.5) / src);
}";

// xBR (the idea: Hyllian's): where the colours around a pixel's corner make a diagonal edge, the
// corner takes the neighbour's colour along a smoothed line (45 degrees or the two flatter slopes).
const XBR_FS: &str = "#version 330 core
uniform sampler2D tex; uniform vec2 src; uniform vec2 dst; in vec2 uv; out vec4 o;
ivec2 base; ivec2 dir;
vec3 px(int x, int y) { return texelFetch(tex, clamp(base + ivec2(x, y) * dir, ivec2(0), ivec2(src) - 1), 0).rgb; }
float d(vec3 a, vec3 b) {
    vec3 v = abs(a - b);
    return dot(v, vec3(0.30, 0.59, 0.11)) * 4.0 + abs(v.r - v.g) + abs(v.b - v.g);
}
bool ne(vec3 a, vec3 b) { return any(greaterThan(abs(a - b), vec3(0.004))); }
void main() {
    vec2 p = uv * src;
    base = ivec2(floor(p));
    vec2 f = fract(p) - 0.5;
    float aa = 0.5 * src.y / dst.y + 0.02;
    dir = ivec2(1);
    vec3 E = px(0, 0), res = E;
    for (int k = 0; k < 4; k++) {
        dir = ivec2((k & 1) == 0 ? 1 : -1, (k & 2) == 0 ? 1 : -1);
        vec3 F = px(1, 0), H = px(0, 1), I = px(1, 1), B = px(0, -1), D = px(-1, 0);
        if (!ne(E, F) || !ne(E, H)) continue;
        vec3 C = px(1, -1), G = px(-1, 1);
        float along = d(E, C) + d(E, G) + d(I, px(0, 2)) + d(I, px(2, 0)) + 4.0 * d(H, F);
        float across = d(H, D) + d(H, px(1, 2)) + d(F, px(2, 1)) + d(F, B) + 4.0 * d(E, I);
        if (along >= across) continue;
        vec2 q = f * vec2(dir);
        float a = smoothstep(0.5 - aa, 0.5 + aa, q.x + q.y);
        float fg = d(F, G), hc = d(H, C);
        if (2.0 * fg <= hc && ne(E, G) && ne(D, G)) a = max(a, smoothstep(0.25 - aa, 0.25 + aa, 0.5 * q.x + q.y));
        if (fg >= 2.0 * hc && ne(E, C) && ne(B, C)) a = max(a, smoothstep(0.25 - aa, 0.25 + aa, q.x + 0.5 * q.y));
        res = mix(res, d(E, F) <= d(E, H) ? F : H, a);
    }
    o = vec4(res, 1.0);
}";

const CRT_FS: &str = "#version 330 core
uniform sampler2D tex; uniform vec2 src; uniform vec2 dst; in vec2 uv; out vec4 o;
vec3 lin(vec2 p) { vec3 c = texture(tex, p).rgb; return c * c; }
void main() {
    vec2 c = uv * 2.0 - 1.0;
    c *= 1.0 + vec2(c.y * c.y, c.x * c.x) * vec2(0.022, 0.030);
    vec2 p = c * 0.5 + 0.5;
    if (any(lessThan(p, vec2(0.0))) || any(greaterThan(p, vec2(1.0)))) { o = vec4(0.0, 0.0, 0.0, 1.0); return; }
    vec2 px = 1.0 / src;
    vec3 col = lin(p) * 0.6 + (lin(p - vec2(px.x * 0.35, 0.0)) + lin(p + vec2(px.x * 0.35, 0.0))) * 0.2;
    vec3 halo = vec3(0.0);
    for (int k = 0; k < 8; k++) {
        float a = float(k) * 0.785398;
        halo += lin(p + vec2(cos(a), sin(a)) * px * 2.2);
    }
    col += halo * 0.02;
    // Scanlines: a beam per source row, wider where it is brighter; none when rows are too small.
    float rows = dst.y / src.y;
    float y = fract(p.y * src.y) - 0.5;
    float lum = max(col.r, max(col.g, col.b));
    float w = mix(0.28, 0.48, sqrt(clamp(lum, 0.0, 1.0)));
    float beam = exp(-y * y / (2.0 * w * w)) * 1.38;
    col *= mix(1.0, beam, clamp((rows - 1.5) / 1.5, 0.0, 1.0));
    // An aperture grille of three window pixels.
    int m = int(mod(gl_FragCoord.x, 3.0));
    vec3 mask = vec3(0.86);
    mask[m] = 1.14;
    col *= mix(vec3(1.0), mask, clamp((rows - 2.0) / 2.0, 0.0, 1.0));
    vec2 v = p * (1.0 - p);
    col *= clamp(pow(v.x * v.y * 24.0, 0.18), 0.0, 1.0);
    o = vec4(sqrt(clamp(col, 0.0, 1.0)), 1.0);
}";

struct Gl {
    gl: glow::Context,
    window: Window,
    _ctx: sdl2::video::GLContext,
    progs: [glow::Program; 4],
    vao: glow::VertexArray,
    tex: glow::Texture,
    /// The CRT shader's input.
    fbo: glow::Framebuffer,
    mid: glow::Texture,
    mid_size: (i32, i32),
    tmp: (Vec<u8>, Vec<u8>),
}

struct Soft {
    canvas: sdl2::render::WindowCanvas,
}

enum Kind {
    Gl(Box<Gl>),
    Soft(Soft),
}

pub struct Output {
    kind: Kind,
    /// The last picture's place in the window, in window coordinates, and its size.
    place: (f32, f32, f32, f32),
    frame: (i32, i32),
}

fn program(gl: &glow::Context, fs: &str) -> Result<glow::Program, String> {
    unsafe {
        let p = gl.create_program()?;
        for (kind, src) in [(glow::VERTEX_SHADER, VS), (glow::FRAGMENT_SHADER, fs)] {
            let s = gl.create_shader(kind)?;
            gl.shader_source(s, src);
            gl.compile_shader(s);
            if !gl.get_shader_compile_status(s) {
                return Err(gl.get_shader_info_log(s));
            }
            gl.attach_shader(p, s);
        }
        gl.link_program(p);
        if !gl.get_program_link_status(p) {
            return Err(gl.get_program_info_log(p));
        }
        Ok(p)
    }
}

impl Gl {
    fn new(video: &sdl2::VideoSubsystem, window: Window) -> Result<Gl, (Window, String)> {
        let ctx = match window.gl_create_context() {
            Ok(c) => c,
            Err(e) => return Err((window, e)),
        };
        let _ = video.gl_set_swap_interval(1);
        let gl = unsafe { glow::Context::from_loader_function(|s| video.gl_get_proc_address(s) as *const _) };
        let make = || -> Result<_, String> {
            let progs = [program(&gl, PLAIN_FS)?, program(&gl, SHARP_FS)?, program(&gl, XBR_FS)?, program(&gl, CRT_FS)?];
            unsafe { Ok((progs, gl.create_vertex_array()?, gl.create_texture()?, gl.create_framebuffer()?, gl.create_texture()?)) }
        };
        match make() {
            Ok((progs, vao, tex, fbo, mid)) => {
                Ok(Gl { gl, window, _ctx: ctx, progs, vao, tex, fbo, mid, mid_size: (0, 0), tmp: Default::default() })
            }
            Err(e) => {
                drop(ctx);
                Err((window, e))
            }
        }
    }

    unsafe fn filter(&self, linear: bool) {
        let f = if linear { glow::LINEAR } else { glow::NEAREST } as i32;
        unsafe {
            for (k, v) in [
                (glow::TEXTURE_MIN_FILTER, f),
                (glow::TEXTURE_MAG_FILTER, f),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32),
            ] {
                self.gl.tex_parameter_i32(glow::TEXTURE_2D, k, v);
            }
        }
    }

    unsafe fn pass(&self, prog: glow::Program, rect: [f32; 4], src: (i32, i32), dst: (i32, i32)) {
        let gl = &self.gl;
        unsafe {
            gl.use_program(Some(prog));
            let u = |n: &str| gl.get_uniform_location(prog, n);
            gl.uniform_4_f32(u("rect").as_ref(), rect[0], rect[1], rect[2], rect[3]);
            gl.uniform_2_f32(u("src").as_ref(), src.0 as f32, src.1 as f32);
            gl.uniform_2_f32(u("dst").as_ref(), dst.0 as f32, dst.1 as f32);
            gl.uniform_1_i32(u("tex").as_ref(), 0);
            gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        }
    }

    /// Draws into the back buffer; (x, y, w, h) is the picture's place in drawable pixels.
    fn draw(&mut self, p: &Pic, look: &Look, (x, y, w, h): (i32, i32, i32, i32)) {
        let (dw, dh) = self.window.drawable_size();
        let mut tmp = std::mem::take(&mut self.tmp);
        let scale4 = look.upscale == SCALE4X && p.w * 4 <= 4096;
        let (data, tw, th): (&[u8], i32, i32) = if scale4 {
            crate::scale::scale4(p.rgb, p.w as usize, p.h as usize, &mut tmp.0, &mut tmp.1);
            (&tmp.1, p.w * 4, p.h * 4)
        } else {
            (p.rgb, p.w, p.h)
        };
        let (prog, linear) = match look.upscale {
            SHARP | SCALE4X => (self.progs[1], true),
            XBR => (self.progs[2], false),
            _ => (self.progs[0], false),
        };
        let gl = &self.gl;
        unsafe {
            gl.bind_vertex_array(Some(self.vao));
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.tex));
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.tex_image_2d(
                glow::TEXTURE_2D, 0, glow::RGB8 as i32, tw, th, 0, glow::RGB, glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(data)),
            );
            self.filter(linear);
            gl.clear_color(0.0, 0.0, 0.0, 1.0);
            let full = [-1.0, -1.0, 1.0, 1.0];
            if look.crt {
                if self.mid_size != (w, h) {
                    gl.bind_texture(glow::TEXTURE_2D, Some(self.mid));
                    gl.tex_image_2d(
                        glow::TEXTURE_2D, 0, glow::RGB8 as i32, w, h, 0, glow::RGB, glow::UNSIGNED_BYTE,
                        glow::PixelUnpackData::Slice(None),
                    );
                    self.filter(true);
                    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
                    gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(self.mid), 0);
                    gl.bind_texture(glow::TEXTURE_2D, Some(self.tex));
                    self.mid_size = (w, h);
                }
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
                gl.viewport(0, 0, w, h);
                // Upside down, so that the second pass reads it like an uploaded picture.
                self.pass(prog, [-1.0, 1.0, 1.0, -1.0], (tw, th), (w, h));
                gl.bind_framebuffer(glow::FRAMEBUFFER, None);
                gl.bind_texture(glow::TEXTURE_2D, Some(self.mid));
                gl.viewport(0, 0, dw as i32, dh as i32);
                gl.clear(glow::COLOR_BUFFER_BIT);
                gl.viewport(x, dh as i32 - y - h, w, h);
                self.pass(self.progs[3], full, (p.w, p.h), (w, h));
            } else {
                gl.bind_framebuffer(glow::FRAMEBUFFER, None);
                gl.viewport(0, 0, dw as i32, dh as i32);
                gl.clear(glow::COLOR_BUFFER_BIT);
                gl.viewport(x, dh as i32 - y - h, w, h);
                self.pass(prog, full, (tw, th), (w, h));
            }
        }
        self.tmp = tmp;
    }
}

/// The picture's place in a (dw, dh) drawable.
fn fit(p: &Pic, look: &Look, dw: i32, dh: i32) -> (f32, (i32, i32, i32, i32)) {
    let mut k = (dw as f32 / p.frame.0 as f32).min(dh as f32 / p.frame.1 as f32);
    if look.integer && k >= 1.0 {
        k = k.floor();
    }
    let (w, h) = ((p.size.0 as f32 * k).round() as i32, (p.size.1 as f32 * k).round() as i32);
    (k, ((dw - w) / 2, (dh - h) / 2, w.max(1), h.max(1)))
}

impl Output {
    pub fn new(video: &sdl2::VideoSubsystem, title: &str, w: u32, h: u32, hidden: bool) -> Result<Output, Box<dyn Error>> {
        let build = |opengl: bool| {
            let mut b = video.window(title, w, h);
            b.position_centered().resizable().allow_highdpi();
            if opengl {
                b.opengl();
            }
            if hidden {
                b.hidden();
            }
            b.build()
        };
        let attr = video.gl_attr();
        attr.set_context_profile(sdl2::video::GLProfile::Core);
        attr.set_context_version(3, 3);
        let gl = match std::env::var_os("ET_NOGL") {
            Some(_) => Err("ET_NOGL".to_string()),
            None => build(true).map_err(|e| e.to_string()).and_then(|win| Gl::new(video, win).map_err(|(_, e)| e)),
        };
        let kind = match gl {
            Ok(g) => Kind::Gl(Box::new(g)),
            Err(e) => {
                eprintln!("no OpenGL ({e}): no shaders");
                Kind::Soft(Soft { canvas: build(false)?.into_canvas().present_vsync().build()? })
            }
        };
        Ok(Output { kind, place: (0.0, 0.0, w as f32, h as f32), frame: (crate::W, crate::H) })
    }

    pub fn window_mut(&mut self) -> &mut Window {
        match &mut self.kind {
            Kind::Gl(g) => &mut g.window,
            Kind::Soft(s) => s.canvas.window_mut(),
        }
    }

    /// A window position (as mouse events give it) in the picture's pixels.
    pub fn to_picture(&self, x: i32, y: i32) -> (i32, i32) {
        let (px, py, pw, ph) = self.place;
        let fx = (x as f32 - px) / pw.max(1.0) * self.frame.0 as f32;
        let fy = (y as f32 - py) / ph.max(1.0) * self.frame.1 as f32;
        (fx.floor() as i32, fy.floor() as i32)
    }

    pub fn present(&mut self, p: &Pic, look: &Look) -> Result<(), Box<dyn Error>> {
        self.draw(p, look)?;
        match &mut self.kind {
            Kind::Gl(g) => g.window.gl_swap_window(),
            Kind::Soft(s) => s.canvas.present(),
        }
        Ok(())
    }

    fn draw(&mut self, p: &Pic, look: &Look) -> Result<(), Box<dyn Error>> {
        let (ww, _) = self.window_mut().size();
        let (dw, dh) = self.window_mut().drawable_size();
        let (k, rect) = fit(p, look, dw as i32, dh as i32);
        // The whole picture's place, in window coordinates.
        let d = ww as f32 / dw.max(1) as f32;
        let (fw, fh) = (p.frame.0 as f32 * k, p.frame.1 as f32 * k);
        self.place = ((dw as f32 - fw) / 2.0 * d, (dh as f32 - fh) / 2.0 * d, fw * d, fh * d);
        self.frame = p.frame;
        match &mut self.kind {
            Kind::Gl(g) => g.draw(p, look, rect),
            Kind::Soft(s) => {
                let tc = s.canvas.texture_creator();
                let mut t = tc.create_texture_streaming(PixelFormatEnum::RGB24, p.w as u32, p.h as u32)?;
                t.update(None, p.rgb, p.w as usize * 3)?;
                s.canvas.set_draw_color(sdl2::pixels::Color::BLACK);
                s.canvas.clear();
                s.canvas.copy(&t, None, Some(sdl2::rect::Rect::new(rect.0, rect.1, rect.2 as u32, rect.3 as u32)))?;
            }
        }
        Ok(())
    }

    /// The drawn picture read back as RGB rows from the top (for tests).
    fn read(&mut self, p: &Pic, look: &Look) -> Result<(i32, i32, Vec<u8>), Box<dyn Error>> {
        self.draw(p, look)?;
        let (w, h) = self.window_mut().drawable_size();
        let Kind::Gl(g) = &mut self.kind else { return Err("no OpenGL".into()) };
        let mut px = vec![0u8; (w * h * 3) as usize];
        unsafe {
            g.gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
            g.gl.read_pixels(0, 0, w as i32, h as i32, glow::RGB, glow::UNSIGNED_BYTE, glow::PixelPackData::Slice(Some(&mut px)));
            let e = g.gl.get_error();
            if e != glow::NO_ERROR {
                return Err(format!("OpenGL error {e:#x}").into());
            }
        }
        let row = w as usize * 3;
        let flipped = px.chunks_exact(row).rev().flatten().copied().collect();
        Ok((w as i32, h as i32, flipped))
    }
}

/// Debug: `ET_GL_SHOT="in.ppm out.ppm upscale crt integer W H"` draws a PPM through the shaders in
/// a hidden window.
pub fn shot(spec: &str) -> Result<(), Box<dyn Error>> {
    let w: Vec<&str> = spec.split_whitespace().collect();
    let n = |i: usize| w.get(i).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
    let b = std::fs::read(w[0])?;
    let text: Vec<String> = b.splitn(4, |&c| c == b'\n').take(3).map(|l| String::from_utf8_lossy(l).into_owned()).collect();
    let size: Vec<i32> = text[1].split_whitespace().filter_map(|v| v.parse().ok()).collect();
    let rgb = &b[b.len() - (size[0] * size[1] * 3) as usize..];
    let sdl = sdl2::init()?;
    let mut out = Output::new(&sdl.video()?, "shot", n(5) as u32, n(6) as u32, true)?;
    let pic = Pic { rgb, w: size[0], h: size[1], size: (size[0], size[1]), frame: (size[0], size[1]) };
    let look = Look { upscale: n(2), crt: n(3) != 0, integer: n(4) != 0 };
    let (dw, dh, px) = out.read(&pic, &look)?;
    let mut f = format!("P6\n{dw} {dh}\n255\n").into_bytes();
    f.extend(px);
    std::fs::write(w[1], f)?;
    println!("drawn at {dw}x{dh}");
    Ok(())
}
