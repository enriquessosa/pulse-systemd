use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

#[derive(Default)]
pub struct App {
    pub show_help: bool,
    pub should_quit: bool,
}

impl App {
    pub fn handle_event(&mut self, event: Event) {
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.should_quit = true;
                }
                KeyCode::Char('?') => self.show_help = !self.show_help,
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    #[test]
    fn help_toggle_and_quit() {
        let mut app = App::default();
        for expected in [true, false] {
            app.handle_event(Event::Key(KeyEvent::new(
                KeyCode::Char('?'),
                KeyModifiers::NONE,
            )));
            assert_eq!(app.show_help, expected);
            assert!(!app.should_quit);
        }
        app.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )));
        assert!(app.should_quit);
    }

    #[test]
    fn releases_and_resize_do_not_quit() {
        let mut app = App::default();
        let mut key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        key.kind = KeyEventKind::Release;
        app.handle_event(Event::Key(key));
        app.handle_event(Event::Resize(1, 1));
        assert!(!app.should_quit);
        app.handle_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(app.should_quit);
    }
}
