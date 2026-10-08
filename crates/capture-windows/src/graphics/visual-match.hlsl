Texture2D<float4> Source : register(t0);
cbuffer Conversion : register(b0) { float InputLinearScale; float3 Padding; };
struct VertexOutput { float4 Position : SV_POSITION; };
VertexOutput VSMain(uint id : SV_VertexID) {
    VertexOutput output;
    float2 position = float2((id << 1) & 2, id & 2);
    output.Position = float4(position * float2(2, -2) + float2(-1, 1), 0, 1);
    return output;
}
float ToneMap(float value) {
    if (value <= 0) return 0;
    if (value <= 1) {
        float transition = saturate((value - 0.75) / 0.25);
        transition = transition * transition * (3 - 2 * transition);
        return value * lerp(1, 0.92, transition);
    }
    return 1 - 0.08 / value;
}
float ToSrgb(float value) {
    return value <= 0.0031308 ? value * 12.92 : 1.055 * pow(value, 1.0 / 2.4) - 0.055;
}
float4 PSMain(VertexOutput input) : SV_TARGET {
    float4 sampled = Source.Load(int3(int2(input.Position.xy), 0));
    float3 mapped = float3(
        ToSrgb(ToneMap(sampled.r * InputLinearScale)),
        ToSrgb(ToneMap(sampled.g * InputLinearScale)),
        ToSrgb(ToneMap(sampled.b * InputLinearScale)));
    return float4(saturate(mapped), saturate(sampled.a));
}
