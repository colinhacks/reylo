select id from profiles where :tag member of (doc->'$.tags')
