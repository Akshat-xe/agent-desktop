import SwiftUI

struct RefChurnView: View {
    @State private var filtered = false
    @State private var alpha = "Alpha"
    @State private var beta = "Beta"
    @State private var stable = "Stable value"
    @State private var replaced = false
    @State private var swapped = false
    @State private var duplicated = false
    @State private var duplicate = "Duplicate value"

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Button(filtered ? "Restore rows" : "Filter rows") {
                filtered.toggle()
            }
            .accessibilityIdentifier("filter-rows")

            HStack {
                Button("Replace rows") {
                    replaced = true
                    beta = "Alpha"
                }
                .accessibilityIdentifier("replace-rows")
                Button("Swap rows") {
                    swapped = true
                }
                .accessibilityIdentifier("swap-rows")
                Button("Add duplicate") {
                    duplicated = true
                }
                .accessibilityIdentifier("add-duplicate")
            }

            VStack(alignment: .leading, spacing: 8) {
                if swapped {
                    TextField("", text: $beta)
                        .accessibilityIdentifier("shared-title")
                }
                if !filtered && !replaced {
                    TextField("", text: $alpha)
                        .accessibilityIdentifier("shared-title")
                }
                if !swapped {
                    TextField("", text: $beta)
                        .accessibilityIdentifier("shared-title")
                }
            }
            .frame(width: 300, height: 80, alignment: .topLeading)
            .accessibilityElement(children: .contain)
            .accessibilityLabel("Changing collection")

            TextField("Stable editor", text: $stable)
                .accessibilityIdentifier("stable-editor")
                .accessibilityLabel("Stable editor")
            if duplicated {
                TextField("Stable editor", text: $duplicate)
                    .accessibilityIdentifier("stable-editor")
                    .accessibilityLabel("Stable editor")
            }
            Text("Alpha stored: \(alpha)")
            Text("Beta stored: \(beta)")
            Text("Stable stored: \(stable)")
            Text("Duplicate stored: \(duplicate)")
            Text("Layout: \(replaced ? "replaced" : swapped ? "swapped" : duplicated ? "duplicated" : "original")")
        }
        .padding(24)
        .frame(width: 380, height: 420)
    }
}
