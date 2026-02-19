use crate::domain::entities::SystemMetrics;
use image::ImageEncoder;
use plotters::coord::Shift;
use plotters::prelude::*;
use std::collections::VecDeque;

pub fn generate_svg_string(
    history: &VecDeque<SystemMetrics>,
    current_metrics: &SystemMetrics,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut buffer = String::new();
    {
        let root = SVGBackend::with_string(&mut buffer, (800, 600)).into_drawing_area();
        draw_system_chart(&root, history, current_metrics)?;
        root.present()?;
    }
    Ok(buffer)
}

pub fn generate_png_buffer(
    history: &VecDeque<SystemMetrics>,
    current_metrics: &SystemMetrics,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut buffer = vec![0u8; 800 * 600 * 3];
    {
        let root = BitMapBackend::with_buffer(&mut buffer, (800, 600)).into_drawing_area();
        draw_system_chart(&root, history, current_metrics)?;
        root.present()?;
    }

    // Encode raw buffer to PNG
    let mut png_buffer = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png_buffer);
    encoder.write_image(&buffer, 800, 600, image::ColorType::Rgb8)?;

    Ok(png_buffer)
}

pub fn draw_system_chart<DB: DrawingBackend>(
    root: &DrawingArea<DB, Shift>,
    history: &VecDeque<SystemMetrics>,
    _metrics: &SystemMetrics,
) -> Result<(), Box<dyn std::error::Error>>
where
    <DB as plotters::prelude::DrawingBackend>::ErrorType: 'static,
{
    root.fill(&RGBColor(30, 30, 46))?; // #1e1e2e

    // Register embedded font
    let font_data = include_bytes!("../assets/fonts/Roboto-Regular.ttf");
    let _ = plotters::style::register_font("roboto", FontStyle::Normal, font_data);

    let max_points = 60;

    // Prepare data series
    let disk_data: Vec<(i32, i32)> = history
        .iter()
        .enumerate()
        .map(|(i, m)| (i as i32, m.disk_usage_percent as i32))
        .collect();
    let cpu_data: Vec<(i32, i32)> = history
        .iter()
        .enumerate()
        .map(|(i, m)| (i as i32, m.cpu_usage_percent as i32))
        .collect();
    let ram_data: Vec<(i32, i32)> = history
        .iter()
        .enumerate()
        .map(|(i, m)| (i as i32, m.ram_usage_percent as i32))
        .collect();

    let mut chart = ChartBuilder::on(root)
        .caption(
            "Aegis-RS System Status",
            ("roboto", 30).into_font().color(&RGBColor(205, 214, 244)),
        )
        .margin(10)
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_cartesian_2d(0..max_points, 0..100)?;

    chart
        .configure_mesh()
        .disable_x_mesh()
        .bold_line_style(RGBColor(69, 71, 90).mix(0.3)) // #45475a
        .y_desc("Usage (%)")
        .axis_style(RGBColor(186, 194, 222)) // #bac2de
        .label_style(("roboto", 15).into_font().color(&RGBColor(186, 194, 222)))
        .draw()?;

    // Disk Usage (Blue)
    chart
        .draw_series(LineSeries::new(
            disk_data,
            RGBColor(137, 180, 250).stroke_width(2), // #89b4fa
        ))?
        .label("Disk")
        .legend(|(x, y)| {
            PathElement::new(
                vec![(x, y), (x + 20, y)],
                RGBColor(137, 180, 250).stroke_width(2),
            )
        });

    // CPU Usage (Red)
    chart
        .draw_series(LineSeries::new(cpu_data, RED))?
        .label("CPU")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], RED));

    // RAM Usage (Green)
    chart
        .draw_series(LineSeries::new(ram_data, GREEN))?
        .label("RAM")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], GREEN));

    chart
        .configure_series_labels()
        .background_style(RGBColor(30, 30, 46).mix(0.8))
        .border_style(RGBColor(186, 194, 222))
        .label_font(("roboto", 15).into_font().color(&RGBColor(205, 214, 244)))
        .draw()?;

    Ok(())
}
