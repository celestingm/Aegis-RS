use crate::domain::entities::SystemMetrics;
use plotters::prelude::*;
use plotters::coord::Shift;
use std::collections::VecDeque;


pub fn generate_svg_string(
    history: &VecDeque<u8>,
    metrics: &SystemMetrics,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut buffer = String::new();
    {
        let root = SVGBackend::with_string(&mut buffer, (800, 400)).into_drawing_area();
        draw_system_chart(&root, history, metrics)?;
    }
    Ok(buffer)
}

pub fn generate_png_buffer(
    history: &VecDeque<u8>,
    metrics: &SystemMetrics,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut img_buffer = vec![0u8; 800 * 400 * 3];
    {
        let root = BitMapBackend::with_buffer(&mut img_buffer, (800, 400)).into_drawing_area();
        draw_system_chart(&root, history, metrics)?;
        root.present()?;
    }
    
    // Encode to PNG
    let mut png_data = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png_data);
    let img = image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(800, 400, img_buffer)
        .ok_or("Failed to create image buffer")?;
    
    img.write_with_encoder(encoder)?;
    Ok(png_data)
}

pub fn draw_system_chart<DB: DrawingBackend>(
    root: &DrawingArea<DB, Shift>,
    history: &VecDeque<u8>,
    metrics: &SystemMetrics,
) -> Result<(), Box<dyn std::error::Error>>
where
    <DB as plotters::prelude::DrawingBackend>::ErrorType: 'static,
{
    root.fill(&RGBColor(30, 30, 46))?; // #1e1e2e

    // Register embedded font
    let font_data = include_bytes!("../assets/fonts/Roboto-Regular.ttf");
    plotters::style::register_font("roboto", FontStyle::Normal, font_data).map_err(|_| "Invalid font data".to_string())?;

    let mut chart = ChartBuilder::on(root)
        .caption("Aegis-RS System Status", ("roboto", 30).into_font().color(&RGBColor(205, 214, 244)))
        .margin(10)
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_cartesian_2d(0..60, 0..100)?;

    chart.configure_mesh()
        .disable_x_mesh()
        .bold_line_style(&RGBColor(69, 71, 90).mix(0.3)) // #45475a
        .y_desc("Disk Usage (%)")
        .axis_style(&RGBColor(186, 194, 222)) // #bac2de
        .label_style(("roboto", 15).into_font().color(&RGBColor(186, 194, 222)))
        .draw()?;

    chart.draw_series(LineSeries::new(
        history.iter().enumerate().map(|(x, y)| (x as i32, *y as i32)),
        RGBColor(137, 180, 250).stroke_width(2), // #89b4fa
    ))?;

    chart.draw_series(AreaSeries::new(
        history.iter().enumerate().map(|(x, y)| (x as i32, *y as i32)),
        0,
        &RGBColor(137, 180, 250).mix(0.2),
    ))?;

    // Add current value label
    root.draw(&Text::new(
        format!("Current: {}%", metrics.disk_usage_percent),
        (50, 50),
        ("roboto", 20).into_font().color(&RGBColor(205, 214, 244)),
    ))?;

    Ok(())
}
