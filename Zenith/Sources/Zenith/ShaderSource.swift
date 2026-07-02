let metalShaderSource = """
#include <metal_stdlib>
using namespace metal;

struct Uniforms {
    float2 viewportSize;
};

struct BgVertex {
    float2 position;
    float2 size;
    float4 color;
};

struct BgOut {
    float4 position [[position]];
    float4 color;
};

vertex BgOut bg_vertex(
    uint vertexID [[vertex_id]],
    uint instanceID [[instance_id]],
    constant BgVertex *instances [[buffer(0)]],
    constant Uniforms &uniforms [[buffer(1)]]
) {
    BgVertex inst = instances[instanceID];

    float2 positions[6] = {
        float2(0, 0), float2(1, 0), float2(0, 1),
        float2(1, 0), float2(1, 1), float2(0, 1)
    };

    float2 pos = inst.position + positions[vertexID] * inst.size;
    float2 ndc = (pos / uniforms.viewportSize) * 2.0 - 1.0;
    ndc.y = -ndc.y;

    BgOut out;
    out.position = float4(ndc, 0.0, 1.0);
    out.color = inst.color;
    return out;
}

fragment float4 bg_fragment(BgOut in [[stage_in]]) {
    return in.color;
}

struct GlyphVertex {
    float2 position;
    float2 size;
    float2 texOffset;
    float2 texSize;
    float4 color;
};

struct GlyphOut {
    float4 position [[position]];
    float2 texCoord;
    float4 color;
};

vertex GlyphOut glyph_vertex(
    uint vertexID [[vertex_id]],
    uint instanceID [[instance_id]],
    constant GlyphVertex *instances [[buffer(0)]],
    constant Uniforms &uniforms [[buffer(1)]]
) {
    GlyphVertex inst = instances[instanceID];

    float2 positions[6] = {
        float2(0, 0), float2(1, 0), float2(0, 1),
        float2(1, 0), float2(1, 1), float2(0, 1)
    };

    float2 uv = positions[vertexID];
    float2 pos = inst.position + uv * inst.size;
    float2 ndc = (pos / uniforms.viewportSize) * 2.0 - 1.0;
    ndc.y = -ndc.y;

    GlyphOut out;
    out.position = float4(ndc, 0.0, 1.0);
    out.texCoord = inst.texOffset + uv * inst.texSize;
    out.color = inst.color;
    return out;
}

fragment float4 glyph_fragment(
    GlyphOut in [[stage_in]],
    texture2d<float> atlas [[texture(0)]]
) {
    constexpr sampler s(mag_filter::linear, min_filter::linear);
    float4 texColor = atlas.sample(s, in.texCoord);
    return float4(in.color.rgb, in.color.a * texColor.a);
}
"""
