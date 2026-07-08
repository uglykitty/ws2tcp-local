#include <QApplication>

#include "MainWindow.h"

int main(int argc, char *argv[]) {
  QApplication app(argc, argv);
  QCoreApplication::setOrganizationName("ws2tcp-local");
  QCoreApplication::setApplicationName("ws2tcp-local-qt");

  MainWindow window;
  window.show();

  return app.exec();
}
