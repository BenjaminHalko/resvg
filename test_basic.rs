use usvg::{Tree, Options};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Simple SVG content
    let svg_data = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
        <rect width="100" height="100" fill="red"/>
    </svg>"#;

    let options = Options::default();
    let tree = Tree::from_str(svg_data, &options)?;

    println!("Tree size: {:?}", tree.size());
    println!("Root children count: {}", tree.root().children().len());

    // Test accessing basic properties
    if let Some(usvg::Node::Path(ref path)) = tree.root().children().get(0) {
        println!("Path is visible: {}", path.is_visible());
        if let Some(fill) = path.fill() {
            println!("Path has fill: true");
        } else {
            println!("Path has fill: false");
        }
    }

    println!("Test completed successfully!");
    Ok(())
}