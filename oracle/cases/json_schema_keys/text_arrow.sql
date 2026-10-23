select doc ->> 'name' as name, doc ->> 'kind' as kind, doc ->> 'size' as size, doc ->> 'tags' as tags from docs
