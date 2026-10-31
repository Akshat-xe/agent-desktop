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
    @State private var labeledFiltered = false
    @State private var gamma = "Gamma"
    @State private var delta = "Delta"
    @State private var equalVisible = true
    @State private var equalFirst = "Same"
    @State private var equalSecond = "Same"
    @State private var bounded = "Bounded value"
    @State private var boundedDuplicateVisible = false
    @State private var boundedDuplicate = "Bounded duplicate"
    @State private var twinVisible = true
    @State private var twinPlain = "Twin plain"
    @State private var twinIdentified = "Twin identified"

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
                Button("Remove equal") {
                    equalVisible = false
                }
                .accessibilityIdentifier("remove-equal")
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

            Button("Filter labeled") {
                labeledFiltered = true
            }
            .accessibilityIdentifier("filter-labeled")

            VStack(alignment: .leading, spacing: 8) {
                if !labeledFiltered {
                    TextField("", text: $gamma)
                        .accessibilityIdentifier("labeled-title")
                        .accessibilityLabel("Gamma label")
                }
                TextField("", text: $delta)
                    .accessibilityIdentifier("labeled-title")
                    .accessibilityLabel("Delta label")
            }
            .frame(width: 300, height: 80, alignment: .topLeading)
            .accessibilityElement(children: .contain)
            .accessibilityLabel("Labeled collection")

            VStack(alignment: .leading, spacing: 8) {
                if equalVisible {
                    TextField("", text: $equalFirst)
                        .accessibilityIdentifier("equal-title")
                }
                TextField("", text: $equalSecond)
                    .accessibilityIdentifier("equal-title")
            }
            .frame(width: 300, height: 60, alignment: .topLeading)
            .accessibilityElement(children: .contain)
            .accessibilityLabel("Equal collection")

            TextField("Stable editor", text: $stable)
                .accessibilityIdentifier("stable-editor")
                .accessibilityLabel("Stable editor")
            if duplicated {
                TextField("Stable editor", text: $duplicate)
                    .accessibilityIdentifier("stable-editor")
                    .accessibilityLabel("Stable editor")
            }
            HStack(alignment: .top, spacing: 24) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Alpha stored: \(alpha)")
                    Text("Beta stored: \(beta)")
                    Text("Stable stored: \(stable)")
                    Text("Duplicate stored: \(duplicate)")
                    Text("Gamma stored: \(gamma)")
                    Text("Delta stored: \(delta)")
                    Text("Equal first stored: \(equalFirst)")
                    Text("Equal second stored: \(equalSecond)")
                    Text("Equal layout: \(equalVisible ? "pair" : "removed")")
                    Text("Layout: \(labeledFiltered ? "labeled-filtered" : replaced ? "replaced" : swapped ? "swapped" : duplicated ? "duplicated" : "original")")
                }
                VStack(alignment: .leading, spacing: 12) {
                    VStack(alignment: .leading, spacing: 0) {
                        TextField("", text: $bounded)
                            .accessibilityIdentifier("bounded-editor")
                            .accessibilityLabel("Bounded editor")
                        Spacer(minLength: 0)
                    }
                    .frame(width: 300, height: 40, alignment: .topLeading)
                    .accessibilityElement(children: .contain)
                    .accessibilityLabel("Bounded original bay")

                    VStack(alignment: .leading, spacing: 0) {
                        if boundedDuplicateVisible {
                            TextField("", text: $boundedDuplicate)
                                .accessibilityIdentifier("bounded-editor")
                                .accessibilityLabel("Bounded editor")
                        }
                        Spacer(minLength: 0)
                    }
                    .frame(width: 300, height: 40, alignment: .topLeading)
                    .accessibilityElement(children: .contain)
                    .accessibilityLabel("Bounded duplicate bay")

                    Button("Add bounded duplicate") {
                        boundedDuplicateVisible = true
                    }
                    .accessibilityIdentifier("add-bounded-duplicate")

                    Text("Bounded layout: \(boundedDuplicateVisible ? "duplicated" : "single")")

                    VStack(alignment: .leading, spacing: 8) {
                        if twinVisible {
                            TextField("", text: $twinPlain)
                                .accessibilityLabel("Twin label")
                        }
                        TextField("", text: $twinIdentified)
                            .accessibilityIdentifier("twin-identified")
                            .accessibilityLabel("Twin label")
                    }
                    .frame(width: 300, height: 60, alignment: .topLeading)
                    .accessibilityElement(children: .contain)
                    .accessibilityLabel("Twin collection")

                    Button("Remove plain twin") {
                        twinVisible = false
                    }
                    .accessibilityIdentifier("remove-plain-twin")

                    Text("Twin layout: \(twinVisible ? "pair" : "removed")")
                }
                .frame(width: 300, alignment: .topLeading)
            }
        }
        .padding(24)
        .frame(width: 620, height: 800)
    }
}
