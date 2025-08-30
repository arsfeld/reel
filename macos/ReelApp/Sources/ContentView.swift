import SwiftUI

struct ContentView: View {
    @EnvironmentObject var model: AppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Reel macOS").font(.largeTitle).bold()
            Text(model.buildInfo).font(.footnote).foregroundColor(.secondary)

            if model.initialized {
                Text("Backends: ")
                List(model.backends, id: \ .id) { b in
                    HStack {
                        Text(b.name)
                        Spacer()
                        Text(b.kind).foregroundColor(.secondary)
                    }
                }
                .frame(minHeight: 200)
            } else {
                ProgressView("Initializing…")
            }
        }
        .padding()
        .onAppear {
            model.startup()
        }
    }
}

struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView().environmentObject(AppModel())
    }
}

