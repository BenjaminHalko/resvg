// Generated code for making SVG properties animatable
// This script generates the boilerplate needed to wrap properties with Animatable<T>

// 1. Field declarations (replace the original fields)
pub struct ExampleStruct {
    pub(crate) opacity: Animatable<Opacity>,
    // pub(crate) opacity: Opacity,  // <- Original
    pub(crate) visibility: Animatable<bool>,
    // pub(crate) visibility: bool,  // <- Original
    pub(crate) display: Animatable<Display>,
    // pub(crate) display: Display,  // <- Original
    pub(crate) transform: Animatable<Transform>,
    // pub(crate) transform: Transform,  // <- Original
    pub(crate) translate: Animatable<f32>,
    // pub(crate) translate: f32,  // <- Original
    pub(crate) rotate: Animatable<f32>,
    // pub(crate) rotate: f32,  // <- Original
    pub(crate) scale: Animatable<f32>,
    // pub(crate) scale: f32,  // <- Original
    pub(crate) skewX: Animatable<f32>,
    // pub(crate) skewX: f32,  // <- Original
    pub(crate) skewY: Animatable<f32>,
    // pub(crate) skewY: f32,  // <- Original
    pub(crate) matrix: Animatable<f32>,
    // pub(crate) matrix: f32,  // <- Original
    pub(crate) fill: Animatable<Color>,
    // pub(crate) fill: Color,  // <- Original
    pub(crate) fill_opacity: Animatable<Opacity>,
    // pub(crate) fill_opacity: Opacity,  // <- Original
    pub(crate) fill_rule: Animatable<FillRule>,
    // pub(crate) fill_rule: FillRule,  // <- Original
    pub(crate) stroke: Animatable<Color>,
    // pub(crate) stroke: Color,  // <- Original
    pub(crate) stroke_width: Animatable<StrokeWidth>,
    // pub(crate) stroke_width: StrokeWidth,  // <- Original
    pub(crate) stroke_opacity: Animatable<Opacity>,
    // pub(crate) stroke_opacity: Opacity,  // <- Original
    pub(crate) stroke_dasharray: Animatable<f32>,
    // pub(crate) stroke_dasharray: f32,  // <- Original
    pub(crate) stroke_dashoffset: Animatable<f32>,
    // pub(crate) stroke_dashoffset: f32,  // <- Original
    pub(crate) stroke_linecap: Animatable<LineCap>,
    // pub(crate) stroke_linecap: LineCap,  // <- Original
    pub(crate) stroke_linejoin: Animatable<LineJoin>,
    // pub(crate) stroke_linejoin: LineJoin,  // <- Original
    pub(crate) stroke_miterlimit: Animatable<StrokeMiterlimit>,
    // pub(crate) stroke_miterlimit: StrokeMiterlimit,  // <- Original
    pub(crate) color: Animatable<Color>,
    // pub(crate) color: Color,  // <- Original
    pub(crate) stop_color: Animatable<Color>,
    // pub(crate) stop_color: Color,  // <- Original
    pub(crate) stop_opacity: Animatable<Opacity>,
    // pub(crate) stop_opacity: Opacity,  // <- Original
    pub(crate) flood_color: Animatable<Color>,
    // pub(crate) flood_color: Color,  // <- Original
    pub(crate) flood_opacity: Animatable<Opacity>,
    // pub(crate) flood_opacity: Opacity,  // <- Original
    pub(crate) lighting_color: Animatable<Color>,
    // pub(crate) lighting_color: Color,  // <- Original
    pub(crate) font_size: Animatable<f32>,
    // pub(crate) font_size: f32,  // <- Original
    pub(crate) font_family: Animatable<String>,
    // pub(crate) font_family: String,  // <- Original
    pub(crate) font_weight: Animatable<FontWeight>,
    // pub(crate) font_weight: FontWeight,  // <- Original
    pub(crate) font_style: Animatable<FontStyle>,
    // pub(crate) font_style: FontStyle,  // <- Original
    pub(crate) font_variant: Animatable<FontVariant>,
    // pub(crate) font_variant: FontVariant,  // <- Original
    pub(crate) font_stretch: Animatable<FontStretch>,
    // pub(crate) font_stretch: FontStretch,  // <- Original
    pub(crate) letter_spacing: Animatable<f32>,
    // pub(crate) letter_spacing: f32,  // <- Original
    pub(crate) word_spacing: Animatable<f32>,
    // pub(crate) word_spacing: f32,  // <- Original
    pub(crate) text_decoration: Animatable<TextDecoration>,
    // pub(crate) text_decoration: TextDecoration,  // <- Original
    pub(crate) text_anchor: Animatable<TextAnchor>,
    // pub(crate) text_anchor: TextAnchor,  // <- Original
    pub(crate) baseline_shift: Animatable<BaselineShift>,
    // pub(crate) baseline_shift: BaselineShift,  // <- Original
    pub(crate) d: Animatable<String>,
    // pub(crate) d: String,  // <- Original
    pub(crate) path_length: Animatable<String>,
    // pub(crate) path_length: String,  // <- Original
    pub(crate) x: Animatable<f32>,
    // pub(crate) x: f32,  // <- Original
    pub(crate) y: Animatable<f32>,
    // pub(crate) y: f32,  // <- Original
    pub(crate) width: Animatable<f32>,
    // pub(crate) width: f32,  // <- Original
    pub(crate) height: Animatable<f32>,
    // pub(crate) height: f32,  // <- Original
    pub(crate) cx: Animatable<f32>,
    // pub(crate) cx: f32,  // <- Original
    pub(crate) cy: Animatable<f32>,
    // pub(crate) cy: f32,  // <- Original
    pub(crate) r: Animatable<f32>,
    // pub(crate) r: f32,  // <- Original
    pub(crate) rx: Animatable<f32>,
    // pub(crate) rx: f32,  // <- Original
    pub(crate) ry: Animatable<f32>,
    // pub(crate) ry: f32,  // <- Original
    pub(crate) x1: Animatable<f32>,
    // pub(crate) x1: f32,  // <- Original
    pub(crate) y1: Animatable<f32>,
    // pub(crate) y1: f32,  // <- Original
    pub(crate) x2: Animatable<f32>,
    // pub(crate) x2: f32,  // <- Original
    pub(crate) y2: Animatable<f32>,
    // pub(crate) y2: f32,  // <- Original
    pub(crate) points: Animatable<Vec<f32>>,
    // pub(crate) points: Vec<f32>,  // <- Original
    pub(crate) view_box: Animatable<ViewBox>,
    // pub(crate) view_box: ViewBox,  // <- Original
    pub(crate) preserve_aspect_ratio: Animatable<PreserveAspectRatio>,
    // pub(crate) preserve_aspect_ratio: PreserveAspectRatio,  // <- Original
    pub(crate) gradient_transform: Animatable<Transform>,
    // pub(crate) gradient_transform: Transform,  // <- Original
    pub(crate) gradient_units: Animatable<Units>,
    // pub(crate) gradient_units: Units,  // <- Original
    pub(crate) filter: Animatable<Filter>,
    // pub(crate) filter: Filter,  // <- Original
    pub(crate) filter_units: Animatable<Units>,
    // pub(crate) filter_units: Units,  // <- Original
    pub(crate) primitive_units: Animatable<Units>,
    // pub(crate) primitive_units: Units,  // <- Original
    pub(crate) begin: Animatable<String>,
    // pub(crate) begin: String,  // <- Original
    pub(crate) dur: Animatable<String>,
    // pub(crate) dur: String,  // <- Original
    pub(crate) end: Animatable<String>,
    // pub(crate) end: String,  // <- Original
    pub(crate) repeat_count: Animatable<f32>,
    // pub(crate) repeat_count: f32,  // <- Original
    pub(crate) repeat_dur: Animatable<String>,
    // pub(crate) repeat_dur: String,  // <- Original
    pub(crate) restart: Animatable<Restart>,
    // pub(crate) restart: Restart,  // <- Original
    pub(crate) fill: Animatable<Color>,
    // pub(crate) fill: Color,  // <- Original
    pub(crate) calc_mode: Animatable<CalcMode>,
    // pub(crate) calc_mode: CalcMode,  // <- Original
    pub(crate) values: Animatable<String>,
    // pub(crate) values: String,  // <- Original
    pub(crate) key_times: Animatable<Vec<f32>>,
    // pub(crate) key_times: Vec<f32>,  // <- Original
    pub(crate) key_splines: Animatable<KeySplines>,
    // pub(crate) key_splines: KeySplines,  // <- Original
    pub(crate) from: Animatable<String>,
    // pub(crate) from: String,  // <- Original
    pub(crate) to: Animatable<String>,
    // pub(crate) to: String,  // <- Original
    pub(crate) by: Animatable<String>,
    // pub(crate) by: String,  // <- Original
}

// 2. Getter methods (update existing methods)
    /// Opacity property.
    pub fn opacity(&self) -> f32 {
        self.opacity.resolve()
    }

    /// Visibility property.
    pub fn visibility(&self) -> f32 {
        self.visibility.resolve()
    }

    /// Display property.
    pub fn display(&self) -> f32 {
        self.display.resolve()
    }

    /// Transform property.
    pub fn transform(&self) -> f32 {
        self.transform.resolve()
    }

    /// Translate property.
    pub fn translate(&self) -> f32 {
        self.translate.resolve()
    }

    /// Rotate property.
    pub fn rotate(&self) -> f32 {
        self.rotate.resolve()
    }

    /// Scale property.
    pub fn scale(&self) -> f32 {
        self.scale.resolve()
    }

    /// Skewx property.
    pub fn skewX(&self) -> f32 {
        self.skewX.resolve()
    }

    /// Skewy property.
    pub fn skewY(&self) -> f32 {
        self.skewY.resolve()
    }

    /// Matrix property.
    pub fn matrix(&self) -> f32 {
        self.matrix.resolve()
    }

    /// Fill property.
    pub fn fill(&self) -> f32 {
        self.fill.resolve()
    }

    /// FillOpacity property.
    pub fn fill_opacity(&self) -> f32 {
        self.fill_opacity.resolve()
    }

    /// FillRule property.
    pub fn fill_rule(&self) -> f32 {
        self.fill_rule.resolve()
    }

    /// Stroke property.
    pub fn stroke(&self) -> f32 {
        self.stroke.resolve()
    }

    /// StrokeWidth property.
    pub fn stroke_width(&self) -> f32 {
        self.stroke_width.resolve()
    }

    /// StrokeOpacity property.
    pub fn stroke_opacity(&self) -> f32 {
        self.stroke_opacity.resolve()
    }

    /// StrokeDasharray property.
    pub fn stroke_dasharray(&self) -> f32 {
        self.stroke_dasharray.resolve()
    }

    /// StrokeDashoffset property.
    pub fn stroke_dashoffset(&self) -> f32 {
        self.stroke_dashoffset.resolve()
    }

    /// StrokeLinecap property.
    pub fn stroke_linecap(&self) -> f32 {
        self.stroke_linecap.resolve()
    }

    /// StrokeLinejoin property.
    pub fn stroke_linejoin(&self) -> f32 {
        self.stroke_linejoin.resolve()
    }

    /// StrokeMiterlimit property.
    pub fn stroke_miterlimit(&self) -> f32 {
        self.stroke_miterlimit.resolve()
    }

    /// Color property.
    pub fn color(&self) -> f32 {
        self.color.resolve()
    }

    /// StopColor property.
    pub fn stop_color(&self) -> f32 {
        self.stop_color.resolve()
    }

    /// StopOpacity property.
    pub fn stop_opacity(&self) -> f32 {
        self.stop_opacity.resolve()
    }

    /// FloodColor property.
    pub fn flood_color(&self) -> f32 {
        self.flood_color.resolve()
    }

    /// FloodOpacity property.
    pub fn flood_opacity(&self) -> f32 {
        self.flood_opacity.resolve()
    }

    /// LightingColor property.
    pub fn lighting_color(&self) -> f32 {
        self.lighting_color.resolve()
    }

    /// FontSize property.
    pub fn font_size(&self) -> f32 {
        self.font_size.resolve()
    }

    /// FontFamily property.
    pub fn font_family(&self) -> f32 {
        self.font_family.resolve()
    }

    /// FontWeight property.
    pub fn font_weight(&self) -> f32 {
        self.font_weight.resolve()
    }

    /// FontStyle property.
    pub fn font_style(&self) -> f32 {
        self.font_style.resolve()
    }

    /// FontVariant property.
    pub fn font_variant(&self) -> f32 {
        self.font_variant.resolve()
    }

    /// FontStretch property.
    pub fn font_stretch(&self) -> f32 {
        self.font_stretch.resolve()
    }

    /// LetterSpacing property.
    pub fn letter_spacing(&self) -> f32 {
        self.letter_spacing.resolve()
    }

    /// WordSpacing property.
    pub fn word_spacing(&self) -> f32 {
        self.word_spacing.resolve()
    }

    /// TextDecoration property.
    pub fn text_decoration(&self) -> f32 {
        self.text_decoration.resolve()
    }

    /// TextAnchor property.
    pub fn text_anchor(&self) -> f32 {
        self.text_anchor.resolve()
    }

    /// BaselineShift property.
    pub fn baseline_shift(&self) -> f32 {
        self.baseline_shift.resolve()
    }

    /// D property.
    pub fn d(&self) -> f32 {
        self.d.resolve()
    }

    /// PathLength property.
    pub fn path_length(&self) -> f32 {
        self.path_length.resolve()
    }

    /// X property.
    pub fn x(&self) -> f32 {
        self.x.resolve()
    }

    /// Y property.
    pub fn y(&self) -> f32 {
        self.y.resolve()
    }

    /// Width property.
    pub fn width(&self) -> f32 {
        self.width.resolve()
    }

    /// Height property.
    pub fn height(&self) -> f32 {
        self.height.resolve()
    }

    /// Cx property.
    pub fn cx(&self) -> f32 {
        self.cx.resolve()
    }

    /// Cy property.
    pub fn cy(&self) -> f32 {
        self.cy.resolve()
    }

    /// R property.
    pub fn r(&self) -> f32 {
        self.r.resolve()
    }

    /// Rx property.
    pub fn rx(&self) -> f32 {
        self.rx.resolve()
    }

    /// Ry property.
    pub fn ry(&self) -> f32 {
        self.ry.resolve()
    }

    /// X1 property.
    pub fn x1(&self) -> f32 {
        self.x1.resolve()
    }

    /// Y1 property.
    pub fn y1(&self) -> f32 {
        self.y1.resolve()
    }

    /// X2 property.
    pub fn x2(&self) -> f32 {
        self.x2.resolve()
    }

    /// Y2 property.
    pub fn y2(&self) -> f32 {
        self.y2.resolve()
    }

    /// Points property.
    pub fn points(&self) -> f32 {
        self.points.resolve()
    }

    /// ViewBox property.
    pub fn view_box(&self) -> f32 {
        self.view_box.resolve()
    }

    /// PreserveAspectRatio property.
    pub fn preserve_aspect_ratio(&self) -> f32 {
        self.preserve_aspect_ratio.resolve()
    }

    /// GradientTransform property.
    pub fn gradient_transform(&self) -> f32 {
        self.gradient_transform.resolve()
    }

    /// GradientUnits property.
    pub fn gradient_units(&self) -> f32 {
        self.gradient_units.resolve()
    }

    /// Filter property.
    pub fn filter(&self) -> f32 {
        self.filter.resolve()
    }

    /// FilterUnits property.
    pub fn filter_units(&self) -> f32 {
        self.filter_units.resolve()
    }

    /// PrimitiveUnits property.
    pub fn primitive_units(&self) -> f32 {
        self.primitive_units.resolve()
    }

    /// Begin property.
    pub fn begin(&self) -> f32 {
        self.begin.resolve()
    }

    /// Dur property.
    pub fn dur(&self) -> f32 {
        self.dur.resolve()
    }

    /// End property.
    pub fn end(&self) -> f32 {
        self.end.resolve()
    }

    /// RepeatCount property.
    pub fn repeat_count(&self) -> f32 {
        self.repeat_count.resolve()
    }

    /// RepeatDur property.
    pub fn repeat_dur(&self) -> f32 {
        self.repeat_dur.resolve()
    }

    /// Restart property.
    pub fn restart(&self) -> f32 {
        self.restart.resolve()
    }

    /// Fill property.
    pub fn fill(&self) -> f32 {
        self.fill.resolve()
    }

    /// CalcMode property.
    pub fn calc_mode(&self) -> f32 {
        self.calc_mode.resolve()
    }

    /// Values property.
    pub fn values(&self) -> f32 {
        self.values.resolve()
    }

    /// KeyTimes property.
    pub fn key_times(&self) -> f32 {
        self.key_times.resolve()
    }

    /// KeySplines property.
    pub fn key_splines(&self) -> f32 {
        self.key_splines.resolve()
    }

    /// From property.
    pub fn from(&self) -> f32 {
        self.from.resolve()
    }

    /// To property.
    pub fn to(&self) -> f32 {
        self.to.resolve()
    }

    /// By property.
    pub fn by(&self) -> f32 {
        self.by.resolve()
    }


// 3. Animation accessor methods (add these methods)
    /// Opacity animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_opacity(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.opacity.animated()
    }

    /// Visibility animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_visibility(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.visibility.animated()
    }

    /// Display animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_display(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.display.animated()
    }

    /// Transform animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_transform(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.transform.animated()
    }

    /// Translate animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_translate(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.translate.animated()
    }

    /// Rotate animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_rotate(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.rotate.animated()
    }

    /// Scale animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_scale(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.scale.animated()
    }

    /// Skewx animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_skewX(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.skewX.animated()
    }

    /// Skewy animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_skewY(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.skewY.animated()
    }

    /// Matrix animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_matrix(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.matrix.animated()
    }

    /// Fill animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_fill(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.fill.animated()
    }

    /// FillOpacity animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_fill_opacity(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.fill_opacity.animated()
    }

    /// FillRule animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_fill_rule(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.fill_rule.animated()
    }

    /// Stroke animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_stroke(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.stroke.animated()
    }

    /// StrokeWidth animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_stroke_width(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.stroke_width.animated()
    }

    /// StrokeOpacity animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_stroke_opacity(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.stroke_opacity.animated()
    }

    /// StrokeDasharray animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_stroke_dasharray(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.stroke_dasharray.animated()
    }

    /// StrokeDashoffset animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_stroke_dashoffset(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.stroke_dashoffset.animated()
    }

    /// StrokeLinecap animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_stroke_linecap(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.stroke_linecap.animated()
    }

    /// StrokeLinejoin animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_stroke_linejoin(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.stroke_linejoin.animated()
    }

    /// StrokeMiterlimit animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_stroke_miterlimit(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.stroke_miterlimit.animated()
    }

    /// Color animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_color(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.color.animated()
    }

    /// StopColor animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_stop_color(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.stop_color.animated()
    }

    /// StopOpacity animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_stop_opacity(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.stop_opacity.animated()
    }

    /// FloodColor animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_flood_color(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.flood_color.animated()
    }

    /// FloodOpacity animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_flood_opacity(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.flood_opacity.animated()
    }

    /// LightingColor animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_lighting_color(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.lighting_color.animated()
    }

    /// FontSize animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_font_size(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.font_size.animated()
    }

    /// FontFamily animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_font_family(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.font_family.animated()
    }

    /// FontWeight animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_font_weight(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.font_weight.animated()
    }

    /// FontStyle animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_font_style(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.font_style.animated()
    }

    /// FontVariant animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_font_variant(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.font_variant.animated()
    }

    /// FontStretch animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_font_stretch(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.font_stretch.animated()
    }

    /// LetterSpacing animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_letter_spacing(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.letter_spacing.animated()
    }

    /// WordSpacing animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_word_spacing(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.word_spacing.animated()
    }

    /// TextDecoration animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_text_decoration(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.text_decoration.animated()
    }

    /// TextAnchor animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_text_anchor(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.text_anchor.animated()
    }

    /// BaselineShift animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_baseline_shift(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.baseline_shift.animated()
    }

    /// D animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_d(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.d.animated()
    }

    /// PathLength animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_path_length(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.path_length.animated()
    }

    /// X animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_x(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.x.animated()
    }

    /// Y animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_y(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.y.animated()
    }

    /// Width animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_width(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.width.animated()
    }

    /// Height animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_height(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.height.animated()
    }

    /// Cx animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_cx(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.cx.animated()
    }

    /// Cy animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_cy(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.cy.animated()
    }

    /// R animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_r(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.r.animated()
    }

    /// Rx animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_rx(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.rx.animated()
    }

    /// Ry animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_ry(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.ry.animated()
    }

    /// X1 animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_x1(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.x1.animated()
    }

    /// Y1 animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_y1(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.y1.animated()
    }

    /// X2 animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_x2(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.x2.animated()
    }

    /// Y2 animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_y2(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.y2.animated()
    }

    /// Points animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_points(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.points.animated()
    }

    /// ViewBox animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_view_box(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.view_box.animated()
    }

    /// PreserveAspectRatio animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_preserve_aspect_ratio(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.preserve_aspect_ratio.animated()
    }

    /// GradientTransform animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_gradient_transform(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.gradient_transform.animated()
    }

    /// GradientUnits animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_gradient_units(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.gradient_units.animated()
    }

    /// Filter animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_filter(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.filter.animated()
    }

    /// FilterUnits animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_filter_units(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.filter_units.animated()
    }

    /// PrimitiveUnits animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_primitive_units(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.primitive_units.animated()
    }

    /// Begin animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_begin(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.begin.animated()
    }

    /// Dur animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_dur(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.dur.animated()
    }

    /// End animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_end(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.end.animated()
    }

    /// RepeatCount animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_repeat_count(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.repeat_count.animated()
    }

    /// RepeatDur animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_repeat_dur(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.repeat_dur.animated()
    }

    /// Restart animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_restart(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.restart.animated()
    }

    /// Fill animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_fill(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.fill.animated()
    }

    /// CalcMode animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_calc_mode(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.calc_mode.animated()
    }

    /// Values animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_values(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.values.animated()
    }

    /// KeyTimes animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_key_times(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.key_times.animated()
    }

    /// KeySplines animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_key_splines(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.key_splines.animated()
    }

    /// From animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_from(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.from.animated()
    }

    /// To animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_to(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.to.animated()
    }

    /// By animation data (potentially animated).
    #[cfg(feature = "animation")]
    pub fn animated_by(&self) -> Option<&crate::tree::animation::AnimatedValue<f32>> {
        self.by.animated()
    }

//
// To use this generator:
// 1. Copy the field declarations to replace existing fields
// 2. Copy the getter methods to update existing getter methods
// 3. Copy the animation accessor methods to add new methods
// 4. Adjust the type names as needed for each property
