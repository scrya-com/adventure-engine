//! Embedded WGSL source for the four shaders.
//!
//! Kept in a private module so [`crate::ShaderKind::wgsl`] is the public
//! entry point. Mirrors UE's `ESlateShader` → templated shader policy:
//!   * Sprite   → TSlateElementPS<ESlateShader::Default>
//!   * Multiply → TSlateElementPS<ESlateShader::Multiply>
//!   * Overlay  → TSlateElementPS<ESlateShader::LightBlend>
//!   * Post     → post-process variant (UE blends these via SlatePostProcessor)

/// `Sprite` — vertex tinted, alpha-blended textured quad.
///
/// Bind group 0: uniforms (frame UBO)
/// Bind group 1: texture + sampler
pub const SPRITE: &str = r#"//
struct FrameUniforms {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> frame: FrameUniforms;

struct VsIn {
    @location(0) position: vec2<f32>,
    @location(1) uv:       vec2<f32>,
    @location(2) tint:     vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv:        vec2<f32>,
    @location(1) tint:      vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = frame.view_proj * vec4<f32>(in.position, 0.0, 1.0);
    out.uv   = in.uv;
    out.tint = in.tint;
    return out;
}

@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let sampled = textureSample(tex, samp, in.uv);
    return sampled * in.tint;
}
"#;

/// `Multiply` — multiplies fragment with destination (mask blend).
pub const MULTIPLY: &str = r#"//
struct FrameUniforms {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> frame: FrameUniforms;

struct VsIn {
    @location(0) position: vec2<f32>,
    @location(1) uv:       vec2<f32>,
    @location(2) tint:     vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv:        vec2<f32>,
    @location(1) tint:      vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = frame.view_proj * vec4<f32>(in.position, 0.0, 1.0);
    out.uv   = in.uv;
    out.tint = in.tint;
    return out;
}

@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let s = textureSample(tex, samp, in.uv);
    // multiply: src * dst (alpha = tint.a * s.a)
    return vec4<f32>(s.rgb * in.tint.rgb, s.a * in.tint.a);
}
"#;

/// `Overlay` — additive light pass (clamped).
pub const OVERLAY: &str = r#"//
struct FrameUniforms {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> frame: FrameUniforms;

struct VsIn {
    @location(0) position: vec2<f32>,
    @location(1) uv:       vec2<f32>,
    @location(2) tint:     vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv:        vec2<f32>,
    @location(1) tint:      vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = frame.view_proj * vec4<f32>(in.position, 0.0, 1.0);
    out.uv   = in.uv;
    out.tint = in.tint;
    return out;
}

@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let s = textureSample(tex, samp, in.uv);
    // additive (clamped in-blender)
    let rgb = s.rgb * in.tint.rgb * in.tint.a;
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), s.a);
}
"#;

/// `Post` — fullscreen quad; samples a scene-colour texture and applies
/// per-channel curve (gamma, vignette). One draw per frame at the end.
pub const POST: &str = r#"//
struct FrameUniforms {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> frame: FrameUniforms;

struct VsIn {
    @location(0) position: vec2<f32>,
    @location(1) uv:       vec2<f32>,
    @location(2) tint:     vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv:        vec2<f32>,
    @location(1) tint:      vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = vec4<f32>(in.position, 0.0, 1.0);
    out.uv   = in.uv;
    out.tint = in.tint;
    return out;
}

@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let s = textureSample(tex, samp, in.uv);
    // simple gamma + tint
    let gamma = vec3<f32>(1.0 / 2.2);
    return vec4<f32>(pow(s.rgb, gamma) * in.tint.rgb, s.a * in.tint.a);
}
"#;
