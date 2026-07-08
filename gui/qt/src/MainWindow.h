#ifndef MAINWINDOW_H
#define MAINWINDOW_H

#include <QComboBox>
#include <QLineEdit>
#include <QMainWindow>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QCheckBox>
#include <QSpinBox>
#include <QTimer>

#include "ws2tcp_local_ffi.h"

class MainWindow final : public QMainWindow {
  Q_OBJECT

 public:
  explicit MainWindow(QWidget *parent = nullptr);
  ~MainWindow() override;

 private slots:
  void startProxy();
  void stopProxy();
  void refreshStatus();
  void appendLog(QString message);

 private:
  QByteArray buildConfigJson() const;
  void loadUserSettings();
  void saveUserSettings() const;
  void appendError(const QString &prefix);
  void showError(const QString &message);
  void updateRuntimeStatus(const QString &message);
  void updateRuntimeStatusFromLog(const QString &message);
  static void handleRustLog(const char *message, void *userData);

  Ws2TcpHandle *handle_ = nullptr;
  QLineEdit *listenEdit_ = nullptr;
  QLineEdit *gatewayEdit_ = nullptr;
  QLineEdit *basicAuthEdit_ = nullptr;
  QLineEdit *customRulesEdit_ = nullptr;
  QSpinBox *bufferSizeSpin_ = nullptr;
  QSpinBox *refreshIntervalSpin_ = nullptr;
  QComboBox *proxyModeCombo_ = nullptr;
  QCheckBox *verifyCertificateCheck_ = nullptr;
  QPushButton *startButton_ = nullptr;
  QPushButton *stopButton_ = nullptr;
  QPlainTextEdit *logView_ = nullptr;
  QTimer *statusTimer_ = nullptr;
  bool wasRunning_ = false;
  QString runtimeStatus_;
};

#endif
