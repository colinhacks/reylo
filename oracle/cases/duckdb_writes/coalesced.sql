select id from notes where tag is not distinct from :tag and body = coalesce(:body, body)
