# shit i wanna do for now (this will later become a features list)

- render graph
- forward+
- meshlet based renderer: 
    - fully gpu driven, drawing using indirect mesh tasks with count
    - hierarchical lod selection and culling for meshlets (frustum, backface, hi-z)
    - 🚨 meshlet compression
- hlsl as the shading language
- resource management system that favors minimal loading times during runtime  

# milestones

## week 1

- initialize ash, gpu-allocator 
- get A Fucking Triangle on the screen
- shader reflection (hassle-rs + rspirv-reflect?)

## week 2

- render graph
- resource registry
- custom model format
- gltf loading and meshlet generation
- set up depth prepass

## week 3

- node based scenes, nodes carry properties and are parentable.
- editor: hierarchy and inspector

## week 4

- asset management
- editor: asset browser?