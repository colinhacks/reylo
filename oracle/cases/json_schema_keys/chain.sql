select doc -> 'owner' -> 'id' as owner_id, doc -> 'owner' ->> 'email' as email, doc -> 'meta' ->> 'note' as note, doc -> 'meta' -> 'note' as note_json from docs
