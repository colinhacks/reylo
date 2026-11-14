select id from profiles where :tag::json member of (doc->'$.tags')
