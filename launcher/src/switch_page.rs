//! Who plays: the top bar's quick switch (a menu under the account chip) between the signed-in
//! Microsoft accounts and, MFRE, the offline profiles.
use super::servers_page::Field;
use super::*;

impl Launcher {
    /// The chip's two lines: the name that plays, and how.
    pub(super) fn who_plays(&self) -> (String, &'static str) {
        if let Some(a) = self.accounts.selected_offline() {
            return (a.name.clone(), tr("Offline profile"));
        }
        match self.accounts.selected() {
            Some(a) => (a.name.clone(), tr("Microsoft account")),
            // MFRE: nothing picked yet — an offline name is the way in, no sign-in needed.
            None => (String::new(), tr("Play offline")),
        }
    }

    pub(super) fn play_as(&mut self, uuid: &str) {
        let mut accounts = Accounts::load();
        let done = accounts.play_as(uuid).and_then(|()| accounts.save());
        match done {
            Ok(()) => self.accounts = accounts,
            Err(e) => self.sign_in_state = Some(SignInState::Done(Err(e))),
        }
        self.account_menu = false;
    }

    /// MFRE: add an offline profile with the typed name and play as it (Enter in the name
    /// field, or the button beside it). A problem shows where a sign-in problem would; success
    /// closes the owner screen — Play needs no services for the profile.
    pub(super) fn add_offline_name(&mut self) {
        let name = self.servers.value(Field::OfflineName).trim().to_string();
        let mut accounts = Accounts::load();
        let done = accounts
            .add_offline(&name)
            .map(|_| ())
            .and_then(|()| accounts.save());
        match done {
            Ok(()) => {
                self.accounts = accounts;
                self.owner = Some(true);
                self.sign_in_state = None;
                self.servers.text(Field::OfflineName).set(String::new());
                self.close_owner_screen();
            }
            Err(e) => self.sign_in_state = Some(SignInState::Done(Err(e))),
        }
    }

    /// MFRE: drop an offline profile (and its local skin files); the roster moves on.
    pub(super) fn remove_offline(&mut self, uuid: &str) {
        let mut accounts = Accounts::load();
        let done = accounts
            .remove(account::store(), uuid)
            .and_then(|()| accounts.save());
        match done {
            Ok(()) => {
                self.accounts = accounts;
                self.owner = Some(self.accounts.has_owner(account::store()));
            }
            Err(e) => self.sign_in_state = Some(SignInState::Done(Err(e))),
        }
    }

    /// Everyone the player can play as (each account's UUID and name), with how each signs in.
    /// MFRE: the offline profiles join the list.
    pub(super) fn players(&self) -> Vec<(String, String, &'static str)> {
        self.accounts
            .accounts
            .iter()
            .map(|a| (a.uuid.clone(), a.name.clone(), tr("Microsoft")))
            .chain(
                self.accounts
                    .offline_profiles
                    .iter()
                    .map(|a| (a.uuid.clone(), a.name.clone(), tr("Offline"))),
            )
            .collect()
    }

    /// The menu under the account chip, while open.
    pub(super) fn account_menu(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if !self.account_menu {
            return None;
        }
        let t = self.theme;
        let now = self.accounts.selected().map(|a| a.uuid.clone());
        let mut menu = div()
            .id("account-menu")
            .absolute()
            .top(px(46.))
            .right_0()
            .w(px(280.))
            .p(px(6.))
            .border_2()
            .border_color(t.outline)
            .bg(t.surface)
            .flex()
            .flex_col()
            .occlude()
            .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                this.account_menu = false;
                this.menu_closed_at = Some(event.position);
                cx.notify();
            }));
        for (i, (who, name, how)) in self.players().into_iter().enumerate() {
            let chosen = Some(&who) == now.as_ref();
            menu = menu.child(
                div()
                    .id(SharedString::from(format!("account-pick-{i}")))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .h(px(46.))
                    .px(px(10.))
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .hover(move |s| s.bg(t.plate_soft))
                    .child(
                        self.profile_heads
                            .draw(Some(who.as_str()), 24., self.scale, &t),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(text(theme::LABEL).truncate().text_color(t.ink).child(name))
                            .child(text(theme::CAPTION).text_color(t.muted).child(how)),
                    )
                    .when(chosen, |d| d.child(div().size(px(6.)).bg(t.accent)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.play_as(&who);
                        cx.notify();
                    })),
            );
        }
        menu = menu.child(div().my(px(6.)).h(px(1.)).bg(t.rule)).child(
            div()
                .id("account-manage")
                .flex()
                .items_center()
                .h(px(32.))
                .px(px(10.))
                .rounded(px(theme::RADIUS))
                .text_size(px(theme::LABEL.px))
                .text_color(t.muted)
                .cursor_pointer()
                .hover(move |s| s.bg(t.plate_soft).text_color(t.ink))
                .child(tr("Manage accounts"))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.account_menu = false;
                    this.go(Page::Settings);
                    cx.notify();
                })),
        );
        Some(gpui::deferred(menu).with_priority(1))
    }
}
