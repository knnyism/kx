struct VertexOutput
{
    float4 position : SV_Position;
    float3 color : COLOR0;
};

[outputtopology("triangle")]
[numthreads(3, 1, 1)]
void main(uint thread_id: SV_GroupThreadID,
          out vertices VertexOutput verts[3],
          out indices uint3 tris[1])
{
    SetMeshOutputCounts(3, 1);

    const float2 positions[3] = {
        float2(0.0, -0.5),
        float2(0.5, 0.5),
        float2(-0.5, 0.5),
    };

    const float3 colors[3] = {
        float3(1.0, 0.0, 0.0),
        float3(0.0, 1.0, 0.0),
        float3(0.0, 0.0, 1.0),
    };

    verts[thread_id].position = float4(positions[thread_id], 0.0, 1.0);
    verts[thread_id].color = colors[thread_id];

    if (thread_id == 0)
        tris[0] = uint3(0, 1, 2);
}
