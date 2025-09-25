#!/usr/bin/env python3
"""
Simple code generator for making SVG properties animatable.
This generates the boilerplate code needed to wrap properties with Animatable<T>.
"""

import re

# List of properties that should be animatable
ANIMATABLE_PROPERTIES = [
    # Basic properties
    "opacity", "visibility", "display",

    # Transform properties
    "transform", "translate", "rotate", "scale", "skewX", "skewY", "matrix",

    # Fill properties
    "fill", "fill_opacity", "fill_rule",

    # Stroke properties
    "stroke", "stroke_width", "stroke_opacity", "stroke_dasharray", "stroke_dashoffset",
    "stroke_linecap", "stroke_linejoin", "stroke_miterlimit",

    # Color properties
    "color", "stop_color", "stop_opacity", "flood_color", "flood_opacity",
    "lighting_color",

    # Text properties
    "font_size", "font_family", "font_weight", "font_style", "font_variant",
    "font_stretch", "letter_spacing", "word_spacing", "text_decoration",
    "text_anchor", "baseline_shift",

    # Path properties
    "d", "path_length",

    # Position/size properties
    "x", "y", "width", "height", "cx", "cy", "r", "rx", "ry",
    "x1", "y1", "x2", "y2", "points",

    # View properties
    "view_box", "preserve_aspect_ratio",

    # Gradient properties
    "gradient_transform", "gradient_units",

    # Filter properties (basic ones)
    "filter", "filter_units", "primitive_units",

    # Animation properties
    "begin", "dur", "end", "repeat_count", "repeat_dur", "restart", "fill",
    "calc_mode", "values", "key_times", "key_splines", "from", "to", "by",
]

def snake_to_camel(snake_str):
    """Convert snake_case to CamelCase"""
    components = snake_str.split('_')
    return ''.join(word.capitalize() for word in components)

def snake_to_pascal(snake_str):
    """Convert snake_case to PascalCase"""
    return snake_to_camel(snake_str)

def generate_field_updates():
    """Generate the field declarations with Animatable<T>"""
    result = []

    for prop in ANIMATABLE_PROPERTIES:
        # Convert snake_case to appropriate type name
        if prop in ['opacity', 'fill_opacity', 'stroke_opacity', 'stop_opacity', 'flood_opacity']:
            type_name = 'Opacity'
        elif prop in ['stroke_width']:
            type_name = 'StrokeWidth'
        elif prop in ['stroke_miterlimit']:
            type_name = 'StrokeMiterlimit'
        elif prop in ['font_size']:
            type_name = 'f32'
        elif prop in ['x', 'y', 'width', 'height', 'cx', 'cy', 'r', 'rx', 'ry', 'x1', 'y1', 'x2', 'y2']:
            type_name = 'f32'
        elif prop in ['stroke_dasharray', 'stroke_dashoffset']:
            type_name = 'f32'  # or Vec<f32> for arrays
        elif prop in ['points']:
            type_name = 'Vec<f32>'  # or custom Points type
        elif prop in ['d', 'path_length']:
            type_name = 'String'  # or PathData type
        elif prop in ['transform']:
            type_name = 'Transform'
        elif prop in ['view_box']:
            type_name = 'ViewBox'
        elif prop in ['preserve_aspect_ratio']:
            type_name = 'PreserveAspectRatio'
        elif prop in ['color', 'fill', 'stroke', 'stop_color', 'flood_color', 'lighting_color']:
            type_name = 'Color'
        elif prop in ['visibility']:
            type_name = 'bool'
        elif prop in ['display']:
            type_name = 'Display'
        elif prop in ['fill_rule']:
            type_name = 'FillRule'
        elif prop in ['stroke_linecap']:
            type_name = 'LineCap'
        elif prop in ['stroke_linejoin']:
            type_name = 'LineJoin'
        elif prop in ['font_family']:
            type_name = 'String'
        elif prop in ['font_weight']:
            type_name = 'FontWeight'
        elif prop in ['font_style']:
            type_name = 'FontStyle'
        elif prop in ['font_variant']:
            type_name = 'FontVariant'
        elif prop in ['font_stretch']:
            type_name = 'FontStretch'
        elif prop in ['letter_spacing', 'word_spacing']:
            type_name = 'f32'
        elif prop in ['text_decoration']:
            type_name = 'TextDecoration'
        elif prop in ['text_anchor']:
            type_name = 'TextAnchor'
        elif prop in ['baseline_shift']:
            type_name = 'BaselineShift'
        elif prop in ['filter']:
            type_name = 'Filter'
        elif prop in ['filter_units', 'primitive_units']:
            type_name = 'Units'
        elif prop in ['gradient_transform']:
            type_name = 'Transform'
        elif prop in ['gradient_units']:
            type_name = 'Units'
        elif prop in ['begin', 'dur', 'end']:
            type_name = 'String'  # or Duration type
        elif prop in ['repeat_count']:
            type_name = 'f32'  # or RepeatCount type
        elif prop in ['repeat_dur']:
            type_name = 'String'  # or Duration type
        elif prop in ['restart']:
            type_name = 'Restart'
        elif prop in ['fill']:
            type_name = 'FillMode'  # animation fill
        elif prop in ['calc_mode']:
            type_name = 'CalcMode'
        elif prop in ['values']:
            type_name = 'String'  # or Values type
        elif prop in ['key_times']:
            type_name = 'Vec<f32>'
        elif prop in ['key_splines']:
            type_name = 'KeySplines'
        elif prop in ['from', 'to', 'by']:
            type_name = 'String'  # or specific types
        else:
            type_name = 'f32'  # Default fallback

        result.append(f'    pub(crate) {prop}: Animatable<{type_name}>,')
        result.append(f'    // pub(crate) {prop}: {type_name},  // <- Original')

    return '\n'.join(result)

def generate_getter_updates():
    """Generate the getter method updates"""
    result = []

    for prop in ANIMATABLE_PROPERTIES:
        camel_prop = snake_to_camel(prop)
        type_name = 'f32'  # Simplified for demo

        result.append(f'    /// {camel_prop} property.')
        result.append(f'    pub fn {prop}(&self) -> {type_name} {{')
        result.append(f'        self.{prop}.resolve()')
        result.append('    }')
        result.append('')

    return '\n'.join(result)

def generate_animation_accessors():
    """Generate the animation accessor methods"""
    result = []

    for prop in ANIMATABLE_PROPERTIES:
        camel_prop = snake_to_camel(prop)
        type_name = 'f32'  # Simplified for demo

        result.append(f'    /// {camel_prop} animation data (potentially animated).')
        result.append(f'    #[cfg(feature = "animation")]')
        result.append(f'    pub fn animated_{prop}(&self) -> Option<&crate::tree::animation::AnimatedValue<{type_name}>> {{')
        result.append(f'        self.{prop}.animated()')
        result.append('    }')
        result.append('')

    return '\n'.join(result)

def main():
    print("// Generated code for making SVG properties animatable")
    print("// This script generates the boilerplate needed to wrap properties with Animatable<T>")
    print()

    print("// 1. Field declarations (replace the original fields)")
    print("pub struct ExampleStruct {")
    print(generate_field_updates())
    print("}")
    print()

    print("// 2. Getter methods (update existing methods)")
    print(generate_getter_updates())
    print()

    print("// 3. Animation accessor methods (add these methods)")
    print(generate_animation_accessors())

    print("//")
    print("// To use this generator:")
    print("// 1. Copy the field declarations to replace existing fields")
    print("// 2. Copy the getter methods to update existing getter methods")
    print("// 3. Copy the animation accessor methods to add new methods")
    print("// 4. Adjust the type names as needed for each property")

if __name__ == "__main__":
    main()