-- loses data: drafts
revoke select on drafts from `reader`@`%`;
revoke update (old_title) on posts from `reader`@`%`;
drop table drafts;
-- posts.old_title is renamed title, keeping its rows. If they are two columns, drop the one and add the other instead.
alter table posts rename column old_title to title;
grant update (title) on posts to `reader`@`%`;
