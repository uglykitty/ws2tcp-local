#include "MainWindow.h"

#include <QByteArray>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMessageBox>
#include <QMetaObject>
#include <QSettings>
#include <QStatusBar>
#include <QVBoxLayout>
#include <QWidget>

MainWindow::MainWindow(QWidget *parent) : QMainWindow(parent) {
  handle_ = ws2tcp_handle_new();

  auto *central = new QWidget(this);
  auto *root = new QVBoxLayout(central);
  auto *form = new QFormLayout();

  listenEdit_ = new QLineEdit("127.0.0.1:8000", this);
  gatewayEdit_ = new QLineEdit(this);
  basicAuthEdit_ = new QLineEdit(this);
  customRulesEdit_ = new QLineEdit(this);

  bufferSizeSpin_ = new QSpinBox(this);
  bufferSizeSpin_->setRange(1, 1024 * 1024);
  bufferSizeSpin_->setValue(16 * 1024);

  refreshIntervalSpin_ = new QSpinBox(this);
  refreshIntervalSpin_->setRange(1, 24 * 60 * 60);
  refreshIntervalSpin_->setValue(60);

  proxyModeCombo_ = new QComboBox(this);
  proxyModeCombo_->addItem("global");
  proxyModeCombo_->addItem("auto");

  verifyCertificateCheck_ = new QCheckBox(this);

  form->addRow("Listen", listenEdit_);
  form->addRow("Gateway", gatewayEdit_);
  form->addRow("Basic auth", basicAuthEdit_);
  form->addRow("Custom rules", customRulesEdit_);
  form->addRow("Buffer size", bufferSizeSpin_);
  form->addRow("Rule refresh seconds", refreshIntervalSpin_);
  form->addRow("Proxy mode", proxyModeCombo_);
  form->addRow("Verify TLS certificate", verifyCertificateCheck_);

  auto *buttons = new QHBoxLayout();
  startButton_ = new QPushButton("Start", this);
  stopButton_ = new QPushButton("Stop", this);
  buttons->addWidget(startButton_);
  buttons->addWidget(stopButton_);
  buttons->addStretch();

  logView_ = new QPlainTextEdit(this);
  logView_->setReadOnly(true);

  root->addLayout(form);
  root->addLayout(buttons);
  root->addWidget(logView_);

  setCentralWidget(central);
  setWindowTitle("ws2tcp-local");
  resize(720, 520);

  connect(startButton_, &QPushButton::clicked, this, &MainWindow::startProxy);
  connect(stopButton_, &QPushButton::clicked, this, &MainWindow::stopProxy);

  const int logRc =
      ws2tcp_set_log_callback(&MainWindow::handleRustLog, this,
                              "ws2tcp_local=info,ws2tcp_local_ffi=info");
  if (logRc != WS2TCP_OK) {
    logView_->appendPlainText("Failed to initialize Rust log callback");
  }

  statusTimer_ = new QTimer(this);
  statusTimer_->setInterval(1000);
  connect(statusTimer_, &QTimer::timeout, this, &MainWindow::refreshStatus);
  statusTimer_->start();

  if (handle_ == nullptr) {
    showError("Failed to create ws2tcp handle");
    startButton_->setEnabled(false);
    stopButton_->setEnabled(false);
  }

  loadUserSettings();
  refreshStatus();
}

MainWindow::~MainWindow() {
  saveUserSettings();

  if (handle_ != nullptr) {
    ws2tcp_stop(handle_);
    ws2tcp_handle_free(handle_);
  }
}

void MainWindow::startProxy() {
  if (handle_ == nullptr) {
    showError("Failed to create ws2tcp handle");
    return;
  }
  if (gatewayEdit_->text().trimmed().isEmpty()) {
    showError("Gateway is required");
    return;
  }

  const QByteArray config = buildConfigJson();
  const int rc = ws2tcp_start(handle_, config.constData());
  if (rc == WS2TCP_OK) {
    saveUserSettings();
    updateRuntimeStatus("Starting " + listenEdit_->text().trimmed());
    logView_->appendPlainText("Started");
  } else {
    appendError("Start failed");
  }
  refreshStatus();
}

void MainWindow::stopProxy() {
  if (handle_ == nullptr) {
    return;
  }

  const int rc = ws2tcp_stop(handle_);
  if (rc == WS2TCP_OK) {
    saveUserSettings();
    updateRuntimeStatus("Stopped");
    logView_->appendPlainText("Stopped");
  } else {
    appendError("Stop failed");
  }
  refreshStatus();
}

void MainWindow::refreshStatus() {
  if (handle_ == nullptr) {
    updateRuntimeStatus("Unavailable");
    startButton_->setEnabled(false);
    stopButton_->setEnabled(false);
    return;
  }

  const bool running =
      ws2tcp_status(handle_) == WS2TCP_STATUS_RUNNING;
  startButton_->setEnabled(!running);
  stopButton_->setEnabled(running);

  if (wasRunning_ && !running) {
    const QString error =
        handle_ != nullptr ? QString::fromUtf8(ws2tcp_last_error(handle_))
                           : QString();
    if (!error.isEmpty()) {
      logView_->appendPlainText("Proxy stopped with error: " + error);
      updateRuntimeStatus("Stopped with error");
    } else {
      updateRuntimeStatus("Stopped");
    }
  }
  wasRunning_ = running;
}

void MainWindow::appendLog(QString message) {
  logView_->appendPlainText(message);
  updateRuntimeStatusFromLog(message);
}

QByteArray MainWindow::buildConfigJson() const {
  QJsonObject config;
  config["listen"] = listenEdit_->text().trimmed();
  config["gateway"] = gatewayEdit_->text().trimmed();
  config["buffer_size"] = bufferSizeSpin_->value();
  config["rule_refresh_interval_secs"] = refreshIntervalSpin_->value();
  config["proxy_mode"] = proxyModeCombo_->currentText();
  config["verify_server_certificate"] = verifyCertificateCheck_->isChecked();

  if (!basicAuthEdit_->text().isEmpty()) {
    config["basic_auth"] = basicAuthEdit_->text().trimmed();
  }
  if (!customRulesEdit_->text().isEmpty()) {
    config["custom_domain_rules"] = customRulesEdit_->text().trimmed();
  }

  return QJsonDocument(config).toJson(QJsonDocument::Compact);
}

void MainWindow::loadUserSettings() {
  QSettings settings;

  listenEdit_->setText(
      settings.value("proxy/listen", listenEdit_->text()).toString());
  gatewayEdit_->setText(settings.value("proxy/gateway").toString());
  basicAuthEdit_->setText(settings.value("proxy/basic_auth").toString());
  customRulesEdit_->setText(settings.value("proxy/custom_rules").toString());
  bufferSizeSpin_->setValue(
      settings.value("proxy/buffer_size", bufferSizeSpin_->value()).toInt());
  refreshIntervalSpin_->setValue(settings
                                     .value("proxy/rule_refresh_interval_secs",
                                            refreshIntervalSpin_->value())
                                     .toInt());

  const QString proxyMode =
      settings.value("proxy/proxy_mode", proxyModeCombo_->currentText())
          .toString();
  const int proxyModeIndex = proxyModeCombo_->findText(proxyMode);
  if (proxyModeIndex >= 0) {
    proxyModeCombo_->setCurrentIndex(proxyModeIndex);
  }

  verifyCertificateCheck_->setChecked(
      settings.value("proxy/verify_server_certificate",
                     verifyCertificateCheck_->isChecked())
          .toBool());
}

void MainWindow::saveUserSettings() const {
  QSettings settings;

  settings.setValue("proxy/listen", listenEdit_->text());
  settings.setValue("proxy/gateway", gatewayEdit_->text());
  settings.setValue("proxy/basic_auth", basicAuthEdit_->text());
  settings.setValue("proxy/custom_rules", customRulesEdit_->text());
  settings.setValue("proxy/buffer_size", bufferSizeSpin_->value());
  settings.setValue("proxy/rule_refresh_interval_secs",
                    refreshIntervalSpin_->value());
  settings.setValue("proxy/proxy_mode", proxyModeCombo_->currentText());
  settings.setValue("proxy/verify_server_certificate",
                    verifyCertificateCheck_->isChecked());
}

void MainWindow::appendError(const QString &prefix) {
  const char *error = ws2tcp_last_error(handle_);
  const QString message = prefix + ": " + QString::fromUtf8(error);
  logView_->appendPlainText(message);
  showError(message);
}

void MainWindow::showError(const QString &message) {
  updateRuntimeStatus(message);
  QMessageBox::warning(this, "ws2tcp-local", message);
}

void MainWindow::updateRuntimeStatus(const QString &message) {
  runtimeStatus_ = message;
  statusBar()->showMessage(runtimeStatus_);
}

void MainWindow::updateRuntimeStatusFromLog(const QString &message) {
  const int listenIndex = message.indexOf("listen=");
  if (listenIndex >= 0) {
    const QString rest = message.mid(listenIndex + 7);
    const QString listen = rest.section(' ', 0, 0);
    if (!listen.isEmpty()) {
      updateRuntimeStatus("Running on " + listen);
      return;
    }
  }

  if (message.contains("starting proxy listen=")) {
    const int start = message.indexOf("starting proxy listen=");
    const QString rest = message.mid(start + 22);
    const QString listen = rest.section(' ', 0, 0);
    if (!listen.isEmpty()) {
      updateRuntimeStatus("Starting " + listen);
    }
  } else if (message.contains("proxy task stopped")) {
    updateRuntimeStatus("Stopped");
  } else if (message.contains("proxy task failed")) {
    updateRuntimeStatus("Stopped with error");
  }
}

void MainWindow::handleRustLog(const char *message, void *userData) {
  auto *window = static_cast<MainWindow *>(userData);
  if (window == nullptr || message == nullptr) {
    return;
  }

  const QString copied = QString::fromUtf8(message);
  QMetaObject::invokeMethod(window, "appendLog", Qt::QueuedConnection,
                            Q_ARG(QString, copied));
}
