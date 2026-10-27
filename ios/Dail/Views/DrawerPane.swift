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
                    if !drawer.rail {
                        HStack {
                            Text(verbatim: account.email)
                            Spacer()
                            if let fold = account.fold { chevron(fold) }
                        }
                    }
                }
            }
            Section {
                ForEach(Array(drawer.footer.enumerated()), id: \.offset) { _, footer in item(footer) }
            }
        }
        .listStyle(.sidebar)
    }

    private func item(_ item: DrawerItem) -> some View {
        // The folder tree (MISC-10): indented by depth, a chevron before a parent. The chevron is
        // its own button beside the row's, never inside it.
        HStack(spacing: 4) {
            if !drawer.rail {
                Spacer().frame(width: CGFloat(item.depth) * 16)
                if let fold = item.fold { chevron(fold) }
            }
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
                .contentShape(Rectangle())
            }
            .buttonStyle(.borderless)
            .accessibilityLabel(item.a11y)
            .accessibilityAddTraits(item.selected ? .isSelected : [])
        }
        .listRowBackground(item.selected ? Color.accentColor.opacity(0.15) : nil)
    }

    private func chevron(_ fold: Fold) -> some View {
        SwiftUI.Button { send(.tap(fold.action)) } label: {
            Image(systemName: fold.collapsed ? "chevron.right" : "chevron.down")
                .frame(width: 20)
        }
        .buttonStyle(.borderless)
        .accessibilityLabel(fold.a11y)
        .accessibilityIdentifier("fold-\(fold.a11y)")
    }
}
