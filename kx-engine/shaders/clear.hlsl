[[vk::binding(0, 0)]]
RWTexture2D<float4> output_image;

[numthreads(16, 16, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    uint w, h;
    output_image.GetDimensions(w, h);

    if (id.x < w && id.y < h) {
        output_image[id.xy] = float4(1.0, 0.0, 0.0, 1.0);
    }
}