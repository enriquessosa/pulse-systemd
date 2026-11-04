use crate::app::App;
use ratatui::{
    Frame,
    widgets::{Block, Paragraph, Wrap},
};

pub fn render(frame: &mut Frame, app: &App) {
    let text = if app.show_help {
        "Keyboard help\n\n?  Toggle help\nq / Esc / Ctrl-C  Quit\n\nResize the terminal at any time."
    } else {
        "Read-only systemd service observatory\n\nFoundation preview: service discovery is not implemented yet.\nNo system or user manager has been queried.\n\n? Help   q Quit"
    };
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::bordered().title("Pulse"))
            .wrap(Wrap { trim: false }),
        frame.area(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn renders_help_and_survives_small_sizes() {
        for (width, height) in [(0, 0), (1, 1), (80, 24)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            for show_help in [false, true] {
                terminal
                    .draw(|frame| {
                        render(
                            frame,
                            &App {
                                show_help,
                                ..App::default()
                            },
                        )
                    })
                    .unwrap();
                if width == 80 {
                    let content: String = terminal
                        .backend()
                        .buffer()
                        .content
                        .iter()
                        .map(|c| c.symbol())
                        .collect();
                    assert!(content.contains(if show_help {
                        "Keyboard help"
                    } else {
                        "discovery is not implemented"
                    }));
                }
            }
        }
    }
}
