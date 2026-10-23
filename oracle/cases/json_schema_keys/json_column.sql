select body -> 'title' as title, body ->> 'title' as title_text, body -> 'title2' as other from notes
