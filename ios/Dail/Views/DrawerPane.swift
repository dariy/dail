import SwiftUI

/// Accounts and mailboxes: the modal sheet of compact windows, a pane or a rail otherwise.
struct DrawerPane: View {
    let drawer: Drawer
    let send: (UiEvent) -> Void

    var body: some View {
        List {
            if let all = drawer.allInboxes { item(all) }
            ForEach(Array(drawer.accounts.enumerated()), id: \.offset) { _, account in
                Section {
                    ForEach(Array(account.mailboxes.enumerated()), id: \.offset) { _, mailbox in item(mailbox) }
                } header: {
                    if !drawer.rail { Text(verbatim: account.email) }
                }
            }
            Section {
                ForEach(Array(drawer.footer.enumerated()), id: \.offset) { _, footer in item(footer) }
            }
        }
        .listStyle(.sidebar)
    }

    private func item(_ item: DrawerItem) -> some View {
        SwiftUI.Button { send(.tap(item.action)) } label: {
            HStack {
                Image(systemName: Icons.symbol(item.icon) ?? "folder")
                if !drawer.rail {
                    Text(verbatim: item.label)
                    Spacer()
                    if item.unread > 0 { Text("\(item.unread)").font(.footnote.monospacedDigit()) }
                }
            }
            .fontWeight(item.selected ? .semibold : .regular)
        }
        .listRowBackground(item.selected ? Color.accentColor.opacity(0.15) : nil)
        .accessibilityLabel(item.a11y)
        .accessibilityAddTraits(item.selected ? .isSelected : [])
    }
}
