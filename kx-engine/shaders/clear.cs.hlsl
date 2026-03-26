[[vk::binding(0, 0)]]
[[vk::image_format("rgba16f")]]
RWTexture2D<float4> output_image;

[numthreads(16, 16, 1)]
void main(uint3 thread_id: SV_DispatchThreadID, uint3 group_thread_id: SV_GroupThreadID)
{
    uint2 uv = thread_id.xy;
    uint2 image_size;
    output_image.GetDimensions(image_size.x, image_size.y);

    if (thread_id.x < image_size.x && thread_id.y < image_size.y)
    {
        if (group_thread_id.x == 0 || group_thread_id.y == 0)
            output_image[thread_id.xy] = float4(0.0, 0.0, 0.0, 1.0);
        else
            output_image[thread_id.xy] = float4(float(thread_id.x) / image_size.x,
                                                float(thread_id.y) / image_size.y,
                                                0.0,
                                                1.0);
    }
}
