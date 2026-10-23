select payload -> 'type' as type, payload ->> 'type' as type_text, payload -> 'x' as x, payload ->> 'key' as key from events
