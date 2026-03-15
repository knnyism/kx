# shit i wanna do for now (this will later become a features list)

- depth prepass
- hlsl as the shading language
- meshlet based renderer: 
    - fully gpu driven, drawing using indirect mesh tasks with count
    - hierarchical lod selection and culling for meshlets (frustum, backface, hi-z)
    - 🚨 meshlet compression
- resource management system that favors minimal loading times during runtime  

# milestones

## week 1

- initialize ash, gpu-allocator and egui
- shader reflection (hassle-rs + rspirv-reflect?)
- get A Fucking Triangle on the screen

## week 2

- gltf loading and meshlet generation
- set up depth prepass

## week 3

- node based scenes, nodes carry properties and are parentable.
- editor: hierarchy and inspector

## week 4

- custom model format for fast as fuck asset loading
- asset management
- editor: asset browser?